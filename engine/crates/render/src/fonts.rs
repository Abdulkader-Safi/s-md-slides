//! Fonts for native rendering: bundled Noto fonts, optionally system fonts and extra
//! folders. One set feeds parley (text layout), usvg (text inside Mermaid and math
//! SVG) and krilla (PDF text).

use parley::fontique::{Collection, CollectionOptions};
use parley::{FontContext, LayoutContext};
use resvg::usvg::fontdb;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Noto Sans, Noto Sans Arabic and Noto Sans Mono (SIL Open Font License 1.1, see
/// `assets/fonts/OFL.txt`).
const BUNDLED: &[&[u8]] = &[
    include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSansArabic-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf"),
];

/// Fallback families appended to every font stack, so Latin, Arabic and code always
/// have a font even without system fonts.
pub(crate) const FALLBACK: &str = "'Noto Sans', 'Noto Sans Arabic'";
pub(crate) const MONO_FALLBACK: &str = "'Noto Sans Mono', 'Noto Sans', 'Noto Sans Arabic'";

/// RGBA color carried through parley as each span's brush.
pub(crate) type Brush = [u8; 4];

pub(crate) struct Inner {
    pub fcx: FontContext,
    pub lcx: LayoutContext<Brush>,
    pub db: Arc<fontdb::Database>,
    /// Glyph outlines as SVG path data, keyed by font blob, index, glyph and size.
    pub outlines: HashMap<(u64, u32, u32, u32), String>,
}

/// The fonts a render uses. Build once and reuse across exports.
pub struct Fonts {
    pub(crate) inner: RefCell<Inner>,
}

impl Fonts {
    fn new(system: bool) -> Fonts {
        let mut fcx = FontContext {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: system,
            }),
            source_cache: Default::default(),
        };
        let mut db = fontdb::Database::new();
        if system {
            db.load_system_fonts();
        }
        for data in BUNDLED {
            fcx.collection.register_fonts(data.to_vec().into(), None);
            db.load_font_data(data.to_vec());
        }
        db.set_sans_serif_family("Noto Sans");
        db.set_serif_family("Noto Sans");
        db.set_monospace_family("Noto Sans Mono");
        Fonts {
            inner: RefCell::new(Inner {
                fcx,
                lcx: LayoutContext::new(),
                db: Arc::new(db),
                outlines: HashMap::new(),
            }),
        }
    }

    /// Only the bundled Noto fonts: the same output on every machine.
    pub fn bundled() -> Fonts {
        Fonts::new(false)
    }

    /// Bundled fonts plus the fonts installed on this machine.
    pub fn with_system() -> Fonts {
        Fonts::new(true)
    }

    /// Add every `.ttf`, `.otf`, `.ttc` and `.otc` file in `dir`. Returns how many
    /// files were added.
    pub fn add_dir(&mut self, dir: &Path) -> std::io::Result<usize> {
        let inner = self.inner.get_mut();
        let db = Arc::make_mut(&mut inner.db);
        let mut n = 0;
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                continue;
            }
            let data = std::fs::read(&path)?;
            inner
                .fcx
                .collection
                .register_fonts(data.clone().into(), None);
            db.load_font_data(data);
            n += 1;
        }
        Ok(n)
    }
}
