//! `s-md-slides export` (FR-CLI-02).

use crate::resolver::Folder;
use crate::{CANCELLED, DECK_ERRORS, Format, OK, has_errors, io_fail, load, print};
use s_md_slides::EngineError;
use s_md_slides::export::{Options, presentation_html};
use s_md_slides_render::{Control, Fonts, Frame, gif, jpeg, pdf, png};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set by the first Ctrl-C. Exports stop at the next page or frame and exit 4.
static CANCEL: AtomicBool = AtomicBool::new(false);

fn cancelled() -> bool {
    CANCEL.load(Ordering::Relaxed)
}

/// First Ctrl-C asks the export to stop cleanly; a second one quits at once.
fn watch_ctrl_c() {
    let _ = ctrlc::set_handler(|| {
        if CANCEL.swap(true, Ordering::Relaxed) {
            std::process::exit(130);
        }
        eprintln!("\nStopping after the current page or frame (Ctrl-C again to quit now)");
    });
}

pub struct Args {
    pub deck: PathBuf,
    pub to: Format,
    pub slide: Option<usize>,
    pub scale: f32,
    pub quality: u8,
    pub fps: u32,
    pub embed: bool,
    pub out: Option<PathBuf>,
}

fn ext(f: Format) -> &'static str {
    match f {
        Format::Html => "html",
        Format::Pdf => "pdf",
        Format::Png => "png",
        Format::Jpeg => "jpg",
        Format::Gif => "gif",
    }
}

/// A one-line progress bar on stderr, only when it is a terminal.
fn bar(what: &str) -> impl FnMut(u32, u32) -> Control {
    let tty = std::io::stderr().is_terminal();
    let what = what.to_string();
    move |done, total| {
        if tty {
            let pct = (done * 100).checked_div(total).unwrap_or(100);
            let filled = (pct / 5) as usize;
            eprint!(
                "\r{what} [{}{}] {pct:3}%",
                "#".repeat(filled),
                " ".repeat(20 - filled)
            );
            if done == total {
                eprintln!();
            }
            let _ = std::io::stderr().flush();
        }
        if cancelled() {
            Control::Cancel
        } else {
            Control::Continue
        }
    }
}

pub fn run(a: &Args) -> ExitCode {
    let (deck, mut diags) = match load(&a.deck) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let folder = Folder::for_deck(&a.deck);
    let stem = a
        .deck
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("deck")
        .to_string();
    let n = deck.slides.len();
    if let Some(s) = a.slide
        && (s == 0 || s > n)
    {
        eprintln!("s-md-slides: --slide must be from 1 to {n}");
        return ExitCode::from(2);
    }
    if !(a.scale > 0.0 && a.scale <= 8.0) {
        eprintln!("s-md-slides: --scale must be above 0 and at most 8");
        return ExitCode::from(2);
    }
    watch_ctrl_c();
    let fonts = Fonts::with_system();
    let default_out = |suffix: &str| {
        a.deck
            .with_file_name(format!("{stem}{suffix}.{}", ext(a.to)))
    };
    let mut written = Vec::new();
    let result: Result<(), ExitCode> = (|| {
        let mut write = |path: PathBuf, bytes: &[u8]| -> Result<(), ExitCode> {
            std::fs::write(&path, bytes).map_err(|e| io_fail("can't write", &path, e))?;
            written.push(path);
            Ok(())
        };
        match a.to {
            Format::Html => {
                let r = presentation_html(
                    &deck,
                    &Options {
                        embed: a.embed,
                        resolver: Some(&folder),
                        ..Default::default()
                    },
                );
                diags.extend(r.diagnostics);
                write(
                    a.out.clone().unwrap_or_else(|| default_out("")),
                    r.text.as_bytes(),
                )
            }
            Format::Pdf => {
                let r = pdf(&deck, &folder, &fonts, &mut bar("PDF")).map_err(failed)?;
                diags.extend(r.diagnostics);
                write(a.out.clone().unwrap_or_else(|| default_out("")), &r.bytes)
            }
            Format::Png | Format::Jpeg | Format::Gif => {
                let slides: Vec<usize> = match a.slide {
                    Some(s) => vec![s - 1],
                    None => (0..n).collect(),
                };
                let many = slides.len() > 1;
                for i in slides {
                    if cancelled() {
                        return Err(ExitCode::from(CANCELLED));
                    }
                    let r = match a.to {
                        Format::Png => png(&deck, i, Frame::End, a.scale, &folder, &fonts),
                        Format::Jpeg => {
                            jpeg(&deck, i, Frame::End, a.scale, a.quality, &folder, &fonts)
                        }
                        _ => gif(
                            &deck,
                            i,
                            a.fps,
                            a.scale,
                            true,
                            &folder,
                            &fonts,
                            &mut bar(&format!("GIF slide {}", i + 1)),
                        ),
                    }
                    .map_err(failed)?;
                    diags.extend(r.diagnostics);
                    let path = match (&a.out, many) {
                        (Some(o), true) => {
                            std::fs::create_dir_all(o)
                                .map_err(|e| io_fail("can't create", o, e))?;
                            o.join(format!("{stem}-{}.{}", i + 1, ext(a.to)))
                        }
                        (Some(o), false) => o.clone(),
                        (None, _) => default_out(&format!("-{}", i + 1)),
                    };
                    write(path, &r.bytes)?;
                }
                Ok(())
            }
        }
    })();
    print(&a.deck, &diags);
    if let Err(code) = result {
        return code;
    }
    for p in &written {
        eprintln!("wrote {}", p.display());
    }
    ExitCode::from(if has_errors(&diags) { DECK_ERRORS } else { OK })
}

/// A render that stopped: cancelled exits 4, anything else 3.
fn failed(e: EngineError) -> ExitCode {
    eprintln!("s-md-slides: {}: {}", e.code, e.message);
    ExitCode::from(if e.code == "E-CANCELLED" {
        CANCELLED
    } else {
        crate::IO
    })
}
