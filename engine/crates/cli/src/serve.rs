//! `s-md-slides serve` (FR-CLI-03): preview in the browser with live reload. It reads
//! the deck and its folder and never writes anything.

use crate::resolver::Folder;
use crate::{IO, has_errors, print};
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use s_md_slides::export::{Options, mime_for, presentation_html};
use s_md_slides::{Deck, Diagnostic, Severity};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server};

type Clients = Arc<Mutex<Vec<Box<dyn Write + Send>>>>;

struct Page {
    html: String,
    slides: usize,
    /// Files whose changes trigger a reload: the deck, its assets and its theme.
    watched: HashSet<PathBuf>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Read the deck and build the page. Errors show in an overlay above the slides.
fn build(deck_path: &Path, folder: &Folder) -> Page {
    let text = match std::fs::read_to_string(deck_path) {
        Ok(t) => t,
        Err(e) => {
            return Page {
                html: format!(
                    "<!doctype html><meta charset=\"utf-8\"><title>s-md-slides</title><pre>Can't read {}: {}</pre>",
                    esc(&deck_path.display().to_string()),
                    esc(&e.to_string())
                ),
                slides: 0,
                watched: HashSet::from([deck_path.to_path_buf()]),
            };
        }
    };
    let mut deck = Deck::parse(&text).deck;
    let mut diags: Vec<Diagnostic> = deck.diagnostics().to_vec();
    diags.extend(deck.load_theme(folder));
    let out = presentation_html(
        &deck,
        &Options {
            embed: false,
            resolver: Some(folder),
            live_reload: true,
        },
    );
    diags.extend(out.diagnostics);
    let mut html = out.text;
    let errors: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        let items: String = errors
            .iter()
            .map(|d| {
                format!(
                    "<li>{}:{}: {} {}</li>",
                    d.line,
                    d.column,
                    d.code,
                    esc(&d.message)
                )
            })
            .collect();
        let overlay = format!(
            "<div id=\"smd-diagnostics\" role=\"alert\" style=\"position:fixed;z-index:9;top:12px;left:12px;right:12px;max-height:50vh;overflow:auto;padding:12px 16px;background:#3a0d0d;color:#ffe2e2;font:14px/1.5 ui-monospace,monospace;border-radius:8px\"><strong>{} error(s) in the deck</strong><ul style=\"margin:6px 0 0;padding-left:20px\">{items}</ul></div>",
            errors.len()
        );
        html = html.replacen(
            "<body class=\"smd-loading\">",
            &format!("<body class=\"smd-loading\">{overlay}"),
            1,
        );
    }
    let mut watched = HashSet::from([deck_path.to_path_buf()]);
    for a in deck.assets() {
        watched.insert(folder.root().join(&a.path));
    }
    Page {
        html,
        slides: deck.slides.len(),
        watched,
    }
}

/// `%20` and friends.
fn decode(path: &str) -> String {
    let b = path.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = path
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("valid header")
}

fn broadcast(clients: &Clients, slides: usize) {
    let mut list = clients.lock().unwrap_or_else(|p| p.into_inner());
    list.retain_mut(|w| {
        write!(w, "event: reload\ndata: {slides}\n\n")
            .and_then(|_| w.flush())
            .is_ok()
    });
}

pub fn run(deck: &Path, host: &str, port: u16, open: bool) -> ExitCode {
    let deck = match deck.canonicalize() {
        Ok(p) => p,
        Err(e) => return crate::io_fail("can't open", deck, e),
    };
    let folder = Arc::new(Folder::for_deck(&deck));
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        eprintln!(
            "warning: serving on {host} makes this deck and every file in its folder visible on your network."
        );
    }
    let server = match Server::http((host, port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("s-md-slides: can't listen on {host}:{port}: {e}");
            return ExitCode::from(IO);
        }
    };
    let first = build(&deck, &folder);
    let page = Arc::new(Mutex::new(first));
    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    {
        let text = std::fs::read_to_string(&deck).unwrap_or_default();
        let diags = Deck::parse(&text).diagnostics;
        print(&deck, &diags);
        if has_errors(&diags) {
            eprintln!("The deck has errors; the browser shows them above the slides.");
        }
    }

    // Watch the deck's folder; rebuild when the deck, an asset or the theme changes.
    let (p2, c2, d2, f2) = (page.clone(), clients.clone(), deck.clone(), folder.clone());
    let debouncer = new_debouncer(
        Duration::from_millis(100),
        None,
        move |res: DebounceEventResult| {
            let Ok(events) = res else { return };
            let watched = p2.lock().map(|p| p.watched.clone()).unwrap_or_default();
            // Only content changes count. Linux also reports reads (open, close), and
            // every rebuild reads the deck, so reacting to them would reload forever.
            let changes = events.iter().filter(|e| {
                matches!(
                    e.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                )
            });
            let hit = changes.flat_map(|e| e.paths.iter()).any(|p| {
                watched.contains(p)
                    || watched
                        .iter()
                        .any(|w| w.file_name() == p.file_name() && w.parent() == p.parent())
            });
            if !hit {
                return;
            }
            let next = build(&d2, &f2);
            let slides = next.slides;
            if let Ok(mut g) = p2.lock() {
                *g = next;
            }
            broadcast(&c2, slides);
        },
    );
    let mut debouncer = match debouncer {
        Ok(d) => d,
        Err(e) => {
            eprintln!("s-md-slides: can't watch files: {e}");
            return ExitCode::from(IO);
        }
    };
    if let Err(e) = debouncer.watch(folder.root(), RecursiveMode::Recursive) {
        eprintln!("s-md-slides: can't watch {}: {e}", folder.root().display());
        return ExitCode::from(IO);
    }

    let shown = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let url = format!("http://{shown}:{port}/");
    eprintln!("Serving {} at {url} (Ctrl-C to stop)", deck.display());
    if open {
        let cmd = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(cmd).arg(&url).spawn();
    }

    for req in server.incoming_requests() {
        if *req.method() != Method::Get {
            let _ = req.respond(
                Response::from_string("Method not allowed")
                    .with_status_code(405)
                    .with_header(header("Allow", "GET")),
            );
            continue;
        }
        let path = decode(req.url().split(['?', '#']).next().unwrap_or("/"));
        match path.as_str() {
            "/" | "/index.html" => {
                let html = page.lock().map(|p| p.html.clone()).unwrap_or_default();
                let _ = req.respond(
                    Response::from_string(html)
                        .with_header(header("Content-Type", "text/html; charset=utf-8"))
                        .with_header(header("Cache-Control", "no-store")),
                );
            }
            "/events" => {
                let mut w = req.into_writer();
                let ok = write!(w, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n: connected\n\n")
                    .and_then(|_| w.flush())
                    .is_ok();
                if ok {
                    clients.lock().unwrap_or_else(|p| p.into_inner()).push(w);
                }
            }
            _ => {
                // Files under the deck's folder only: no `..`, no absolute paths, no listing.
                let rel = path.trim_start_matches('/');
                let found = (!rel.split(['/', '\\']).any(|s| s == ".."))
                    .then(|| folder.locate(rel))
                    .flatten();
                match found.and_then(|f| std::fs::read(&f).ok().map(|b| (f, b))) {
                    Some((f, bytes)) => {
                        let _ =
                            req.respond(Response::from_data(bytes).with_header(header(
                                "Content-Type",
                                mime_for(&f.to_string_lossy()),
                            )));
                    }
                    None => {
                        let _ =
                            req.respond(Response::from_string("Not found").with_status_code(404));
                    }
                }
            }
        }
    }
    ExitCode::SUCCESS
}
