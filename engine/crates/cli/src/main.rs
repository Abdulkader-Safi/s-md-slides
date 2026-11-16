//! `s-md-slides`: validate, export, serve and apply for markdown slide decks.
//! Exit codes: 0 ok, 1 the deck has errors, 2 usage, 3 I/O, 4 cancelled.

mod apply;
mod export;
mod resolver;
mod serve;
mod validate;

use clap::{Parser, Subcommand, ValueEnum};
use s_md_slides::{Diagnostic, Severity};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "s-md-slides",
    version,
    about = "Slide decks stored in one readable markdown file"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Format {
    Html,
    Pdf,
    Png,
    Jpeg,
    Gif,
}

#[derive(Subcommand)]
enum Command {
    /// Check a deck and print its problems. Exits 1 when there are errors.
    Validate {
        deck: PathBuf,
        /// Print diagnostics as JSON on stdout.
        #[arg(long)]
        json: bool,
    },
    /// Export a deck to HTML, PDF, PNG, JPEG or GIF.
    Export {
        deck: PathBuf,
        #[arg(long = "to", value_enum)]
        to: Format,
        /// Only this slide (1-based). Image formats export every slide without it.
        #[arg(long)]
        slide: Option<usize>,
        /// Image scale: 1 gives the canvas size, 2 doubles it.
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// JPEG quality, 1 to 100.
        #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        /// GIF frames per second.
        #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u32).range(1..=60))]
        fps: u32,
        /// HTML: put every asset inside the file, so it works offline on its own.
        #[arg(long)]
        embed: bool,
        /// Output file (or folder, for several images). Defaults next to the deck.
        #[arg(short = 'o', long = "out")]
        out: Option<PathBuf>,
    },
    /// Preview a deck in the browser, reloading on every save. Never writes the deck.
    Serve {
        deck: PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 4321)]
        port: u16,
        /// Open the default browser.
        #[arg(long)]
        open: bool,
    },
    /// Apply a JSON op (or a list of ops) to a deck and print the inverse batch.
    Apply {
        deck: PathBuf,
        ops: PathBuf,
        /// Print the inverse without writing the deck.
        #[arg(long)]
        dry_run: bool,
    },
}

pub const OK: u8 = 0;
pub const DECK_ERRORS: u8 = 1;
pub const IO: u8 = 3;
pub const CANCELLED: u8 = 4;

/// An I/O problem: print it and exit 3.
pub fn io_fail(what: &str, path: &Path, e: impl std::fmt::Display) -> ExitCode {
    eprintln!("s-md-slides: {what} {}: {e}", path.display());
    ExitCode::from(IO)
}

/// Print diagnostics as `file:line:col: severity code message` on stderr.
pub fn print(file: &Path, diags: &[Diagnostic]) {
    let color = std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal();
    for d in diags {
        let (label, code) = match d.severity {
            Severity::Error => ("error", "31"),
            Severity::Warning => ("warning", "33"),
            Severity::Info => ("info", "36"),
        };
        let label = if color {
            format!("\x1b[1;{code}m{label}\x1b[0m")
        } else {
            label.to_string()
        };
        eprintln!(
            "{}:{}:{}: {label} {} {}",
            file.display(),
            d.line,
            d.column,
            d.code,
            d.message
        );
    }
}

pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// Read and parse a deck, loading its theme file through the deck folder.
pub fn load(path: &Path) -> Result<(s_md_slides::Deck, Vec<Diagnostic>), ExitCode> {
    let text = std::fs::read_to_string(path).map_err(|e| io_fail("can't read", path, e))?;
    let mut deck = s_md_slides::Deck::parse(&text).deck;
    let folder = resolver::Folder::for_deck(path);
    let mut diags = deck.diagnostics().to_vec();
    diags.extend(deck.load_theme(&folder));
    Ok((deck, diags))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { deck, json } => validate::run(&deck, json),
        Command::Export {
            deck,
            to,
            slide,
            scale,
            quality,
            fps,
            embed,
            out,
        } => export::run(&export::Args {
            deck,
            to,
            slide,
            scale,
            quality,
            fps,
            embed,
            out,
        }),
        Command::Serve {
            deck,
            host,
            port,
            open,
        } => serve::run(&deck, &host, port, open),
        Command::Apply { deck, ops, dry_run } => apply::run(&deck, &ops, dry_run),
    }
}
