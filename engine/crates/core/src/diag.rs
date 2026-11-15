//! Diagnostics: problems found while reading a deck. Parsing never fails; it reports.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// One problem, with a 1-based line and column (column counts characters, not bytes).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable code such as `E-ATTR-VALUE` or `W-OFF-CANVAS`.
    pub code: &'static str,
    /// One English sentence.
    pub message: String,
    pub line: u32,
    pub column: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// 0-based slide index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide: Option<usize>,
}

/// Error returned by operations. The deck is unchanged when one comes back.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide: Option<usize>,
}

impl EngineError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        EngineError {
            code,
            message: message.into(),
            element: None,
            slide: None,
        }
    }
    pub(crate) fn on(mut self, element: &str) -> Self {
        self.element = Some(element.to_string());
        self
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EngineError {}

/// Maps byte offsets to 1-based line and column.
pub(crate) struct LineIndex {
    starts: Vec<usize>,
    bom: usize,
}

impl LineIndex {
    pub(crate) fn new(src: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        let bom = if src.starts_with('\u{feff}') { 3 } else { 0 };
        LineIndex { starts, bom }
    }

    pub(crate) fn pos(&self, src: &str, offset: usize) -> (u32, u32) {
        let offset = offset.min(src.len());
        let line = self.starts.partition_point(|&s| s <= offset) - 1;
        let start = if line == 0 {
            self.bom
        } else {
            self.starts[line]
        };
        let col = src
            .get(start..offset.max(start))
            .map_or(0, |s| s.chars().count());
        (line as u32 + 1, col as u32 + 1)
    }
}
