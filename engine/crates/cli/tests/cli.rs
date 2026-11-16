//! M6: the `s-md-slides` command (FR-CLI-01 to FR-CLI-04).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_s-md-slides"));
    c.env("NO_COLOR", "1");
    c
}

/// A fresh copy of the example decks in a temp folder.
fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("smd-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    let ex = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for e in std::fs::read_dir(&ex).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
    }
    for e in std::fs::read_dir(ex.join("assets")).unwrap().flatten() {
        std::fs::copy(e.path(), dir.join("assets").join(e.file_name())).unwrap();
    }
    dir
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().unwrap()
}

#[test]
fn fr_cli_01_validate_reports_file_line_col_and_exits_1() {
    let dir = workdir("validate");
    let deck = dir.join("broken.md");
    std::fs::write(
        &deck,
        "# fine\n---\n<!-- text id=a x=abc y=0 w=10 h=10 -->\nA\n",
    )
    .unwrap();
    let out = run(&["validate", deck.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains(&format!("{}:3:18: error E-ATTR-VALUE", deck.display())),
        "{err}"
    );
    let ok = run(&["validate", dir.join("hello.md").to_str().unwrap(), "--json"]);
    assert_eq!(ok.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(v["diagnostics"], serde_json::json!([]));
    assert_eq!(
        run(&["validate", dir.join("nope.md").to_str().unwrap()])
            .status
            .code(),
        Some(3),
        "I/O error"
    );
    assert_eq!(run(&["validate"]).status.code(), Some(2), "usage error");
}

#[test]
fn fr_cli_02_export_every_format_and_slide_limits_images() {
    let dir = workdir("export");
    let deck = dir.join("pitch-16x9.md");
    let d = deck.to_str().unwrap();
    for (to, file, magic) in [
        ("html", "pitch-16x9.html", &b"<!doctype html>"[..]),
        ("pdf", "pitch-16x9.pdf", b"%PDF"),
        ("png", "pitch-16x9-3.png", b"\x89PNG"),
        ("jpeg", "pitch-16x9-3.jpg", b"\xFF\xD8"),
        ("gif", "pitch-16x9-3.gif", b"GIF89a"),
    ] {
        let out = run(&["export", d, "--to", to, "--slide", "3"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{to}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let bytes = std::fs::read(dir.join(file)).unwrap_or_else(|_| panic!("{file} missing"));
        assert!(bytes.starts_with(magic), "{file}");
    }
    assert!(
        !dir.join("pitch-16x9-1.png").exists(),
        "--slide limits images to one slide"
    );
    let all = dir.join("all");
    assert_eq!(
        run(&[
            "export",
            d,
            "--to",
            "png",
            "--scale",
            "0.25",
            "-o",
            all.to_str().unwrap()
        ])
        .status
        .code(),
        Some(0)
    );
    let mut names: Vec<_> = std::fs::read_dir(&all)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "pitch-16x9-1.png",
            "pitch-16x9-2.png",
            "pitch-16x9-3.png",
            "pitch-16x9-4.png"
        ]
    );
    let embedded = dir.join("embedded.html");
    run(&[
        "export",
        d,
        "--to",
        "html",
        "--embed",
        "-o",
        embedded.to_str().unwrap(),
    ]);
    assert!(
        std::fs::read_to_string(&embedded)
            .unwrap()
            .contains("data:image/png;base64,")
    );
    assert_eq!(
        run(&["export", d, "--to", "png", "--slide", "9"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn fr_cli_04_apply_changes_only_those_lines() {
    let dir = workdir("apply");
    let deck = dir.join("pitch-16x9.md");
    let before = std::fs::read_to_string(&deck).unwrap();
    let ops = dir.join("ops.json");
    std::fs::write(
        &ops,
        r#"[{"op":"set_geometry","id":"b1","x":200},{"op":"set_geometry","id":"b2","x":200}]"#,
    )
    .unwrap();
    let dry = run(&[
        "apply",
        deck.to_str().unwrap(),
        ops.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(dry.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(&deck).unwrap(),
        before,
        "--dry-run writes nothing"
    );
    let out = run(&["apply", deck.to_str().unwrap(), ops.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = std::fs::read_to_string(&deck).unwrap();
    let changed: Vec<_> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .map(|(_, b)| b)
        .collect();
    assert_eq!(changed.len(), 2, "{changed:?}");
    assert!(changed.iter().all(|l| l.contains("x=200")));
    // The printed inverse undoes it.
    let undo = dir.join("undo.json");
    std::fs::write(&undo, &out.stdout).unwrap();
    assert_eq!(
        run(&["apply", deck.to_str().unwrap(), undo.to_str().unwrap()])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(std::fs::read_to_string(&deck).unwrap(), before);
    std::fs::write(&ops, r#"{"op":"remove_element","id":"nope"}"#).unwrap();
    assert_eq!(
        run(&["apply", deck.to_str().unwrap(), ops.to_str().unwrap()])
            .status
            .code(),
        Some(1)
    );
}

struct Serve(Child);

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn get(port: u16, raw: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn fr_cli_03_serve_reloads_and_never_writes() {
    let dir = workdir("serve");
    let deck = dir.join("pitch-16x9.md");
    std::fs::write(
        dir.join("..")
            .join(format!("secret-{}", std::process::id())),
        "secret",
    )
    .unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mtime = || std::fs::metadata(&deck).unwrap().modified().unwrap();
    let before = mtime();
    let child = bin()
        .args(["serve", deck.to_str().unwrap(), "--port", &port.to_string()])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _guard = Serve(child);
    let started = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "server did not start"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    let page = get(
        port,
        "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    );
    assert!(
        page.starts_with("HTTP/1.1 200") && page.contains("new EventSource(\"/events\")"),
        "live reload script"
    );
    assert!(
        get(
            port,
            "GET /assets/chart.png HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
        )
        .starts_with("HTTP/1.1 200")
    );
    let secret = format!(
        "GET /../secret-{} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        std::process::id()
    );
    assert!(
        get(port, &secret).starts_with("HTTP/1.1 404"),
        "no path traversal"
    );
    assert!(
        get(
            port,
            "GET /%2e%2e/%2e%2e/etc/passwd HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
        )
        .starts_with("HTTP/1.1 404")
    );
    assert!(
        get(
            port,
            "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .starts_with("HTTP/1.1 405")
    );
    assert_eq!(mtime(), before, "serving never touches the deck");

    // Listen for reloads, then save the deck the way Vim does (write a new file, rename).
    let mut events = TcpStream::connect(("127.0.0.1", port)).unwrap();
    events
        .write_all(b"GET /events HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    events
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(events);
    let mut line = String::new();
    while !line.starts_with(": connected") {
        line.clear();
        reader.read_line(&mut line).unwrap();
    }
    std::thread::sleep(Duration::from_millis(300));
    let text = std::fs::read_to_string(&deck)
        .unwrap()
        .replace("# Growth", "# Growth now");
    let tmp = dir.join(".pitch-16x9.md.swp");
    std::fs::write(&tmp, &text).unwrap();
    let saved = Instant::now();
    std::fs::rename(&tmp, &deck).unwrap();
    let mut got = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line.starts_with("event: reload") {
            got = line.clone();
            break;
        }
    }
    let took = saved.elapsed();
    assert!(got.starts_with("event: reload"), "no reload event");
    assert!(took < Duration::from_millis(500), "reload took {took:?}");
    let page = get(
        port,
        "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    );
    assert!(page.contains("Growth now"), "the page shows the new text");
    // One save, one reload: serving the page (which reads files) must not set off more.
    reader
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(1200)))
        .unwrap();
    let mut extra = Vec::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => {
                if line.starts_with("event:") {
                    extra.push(line.clone());
                }
            }
            _ => break,
        }
    }
    assert!(extra.is_empty(), "reload loop: {extra:?}");

    // A broken save shows the diagnostics overlay.
    std::fs::write(&deck, text.replace("x=120 y=80", "x=oops y=80")).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        get(
            port,
            "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
        )
        .contains("smd-diagnostics")
    );
}
