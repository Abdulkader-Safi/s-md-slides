//! Every `markdown` example in `docs/format.md` (and the skill) parses with no
//! diagnostics (FR-AGENT-02, NFR-10).

use s_md_slides::Deck;

/// Fenced blocks tagged `markdown`, honoring longer fences around shorter ones.
fn examples(doc: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut lines = doc.lines().enumerate();
    while let Some((n, line)) = lines.next() {
        let ticks = line.len() - line.trim_start_matches('`').len();
        if ticks >= 3 && line[ticks..].trim() == "markdown" {
            let fence = &line[..ticks];
            let mut body = String::new();
            for (_, l) in lines.by_ref() {
                if l.trim_end() == fence {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            out.push((n + 1, body));
        }
    }
    out
}

fn check(path: &str, min: usize) {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    let doc = std::fs::read_to_string(&full).unwrap_or_else(|_| panic!("{path} missing"));
    let ex = examples(&doc);
    assert!(ex.len() >= min, "{path}: expected at least {min} examples, found {}", ex.len());
    for (line, body) in ex {
        let p = Deck::parse(&body);
        assert!(p.diagnostics.is_empty(), "{path}:{line}: {:?}", p.diagnostics);
        assert_eq!(p.deck.text(), body);
    }
}

#[test]
fn format_doc_examples_parse_cleanly() {
    check("docs/format.md", 15);
}

#[test]
fn skill_examples_parse_cleanly() {
    check("skills/s-md-slides/SKILL.md", 4);
}
