//! `s-md-slides validate` (FR-CLI-01).

use crate::{DECK_ERRORS, OK, has_errors, load, print};
use std::path::Path;
use std::process::ExitCode;

pub fn run(deck: &Path, json: bool) -> ExitCode {
    let (_, diags) = match load(deck) {
        Ok(x) => x,
        Err(code) => return code,
    };
    if json {
        println!(
            "{}",
            serde_json::json!({ "file": deck, "diagnostics": diags })
        );
    } else {
        print(deck, &diags);
    }
    ExitCode::from(if has_errors(&diags) { DECK_ERRORS } else { OK })
}
