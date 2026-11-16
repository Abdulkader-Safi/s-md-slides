//! `s-md-slides apply` (FR-CLI-04): run a JSON op or list of ops, print the inverse
//! batch on stdout, and write the deck unless `--dry-run`.

use crate::{DECK_ERRORS, OK, io_fail, load};
use s_md_slides::Op;
use std::path::Path;
use std::process::ExitCode;

pub fn run(deck_path: &Path, ops_path: &Path, dry_run: bool) -> ExitCode {
    let (mut deck, _) = match load(deck_path) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let json = match std::fs::read_to_string(ops_path) {
        Ok(j) => j,
        Err(e) => return io_fail("can't read", ops_path, e),
    };
    let ops: Vec<Op> = match serde_json::from_str::<Vec<Op>>(&json) {
        Ok(list) => list,
        Err(_) => match serde_json::from_str::<Op>(&json) {
            Ok(op) => vec![op],
            Err(e) => {
                eprintln!(
                    "s-md-slides: {} is not an op or a list of ops: {e}",
                    ops_path.display()
                );
                return ExitCode::from(2);
            }
        },
    };
    let inverse = match deck.apply_batch(ops) {
        Ok(inv) => inv,
        Err(e) => {
            let at = e
                .element
                .as_deref()
                .map(|id| format!(" ({id})"))
                .unwrap_or_default();
            eprintln!(
                "s-md-slides: {}: {} {}{at}",
                deck_path.display(),
                e.code,
                e.message
            );
            return ExitCode::from(DECK_ERRORS);
        }
    };
    println!("{}", serde_json::to_string(&inverse).unwrap_or_default());
    if dry_run {
        return ExitCode::from(OK);
    }
    // Write next to the deck, then rename, so a crash never leaves half a file.
    let tmp = deck_path.with_extension("md.s-md-slides-tmp");
    if let Err(e) = std::fs::write(&tmp, deck.text()) {
        return io_fail("can't write", &tmp, e);
    }
    if let Err(e) = std::fs::rename(&tmp, deck_path) {
        let _ = std::fs::remove_file(&tmp);
        return io_fail("can't replace", deck_path, e);
    }
    ExitCode::from(OK)
}
