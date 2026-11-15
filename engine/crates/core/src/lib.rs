//! `s-md-slides`: a free-canvas slide deck stored in one readable markdown file.
//!
//! Element positions live in one-line HTML comments and the content under each is
//! normal markdown. The engine parses decks, edits them through [`Op`]s that return
//! their own inverse, and saves by splicing only the edited bytes into the original
//! text. It never touches files: hosts pass text in and out.
//!
//! ```
//! use s_md_slides::{Deck, Op};
//!
//! let src = "<!-- text id=t1 x=100 y=100 w=800 h=200 -->\n# Hello\n";
//! let mut deck = Deck::parse(src).deck;
//! let undo = deck.apply(Op::SetGeometry { id: "t1".into(), x: Some(240.0), y: None, w: None, h: None, rot: None }).unwrap();
//! assert_eq!(deck.text(), "<!-- text id=t1 x=240 y=100 w=800 h=200 -->\n# Hello\n");
//! deck.apply(undo).unwrap();
//! assert_eq!(deck.text(), src);
//! ```

pub mod attrs;
mod deck;
pub mod diag;
mod frontmatter;
mod ops;
mod query;

pub use attrs::Attr;
pub use deck::{
    Deck, Element, Kind, Layout, LineEnding, MAX_ATTR_VALUE_BYTES, MAX_ELEMENTS_PER_SLIDE,
    MAX_FILE_BYTES, MAX_SLIDES, Master, MasterSetting, Parsed, Placeholder, Slide, first_strong,
};
pub use diag::{Diagnostic, EngineError, Severity};
pub use frontmatter::{Canvas, Dir, FORMAT_VERSION, FrontMatter};
pub use ops::{Op, ZStep, ZTo};
pub use query::{AssetKind, AssetRef, Hit, HitField};
