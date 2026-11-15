//! Operations: every edit is an `Op`. Each one is validated, applied as one splice into
//! the source text, re-read, checked, and answered with its inverse.
//!
//! ponytail: after each splice the whole text is parsed again rather than shifting
//! stored offsets. That costs O(file size) per op; switch to offset shifting if the
//! NFR-01 benchmark (one op plus save under 5 ms) fails.

use crate::attrs::{fmt_num, quote};
use crate::deck::{Carry, Deck, Element, trim_blank_tail};
use crate::diag::EngineError;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Move, resize or rotate. Only the given fields change.
    SetGeometry {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        w: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        h: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rot: Option<f64>,
    },
    /// Replace an element's markdown body.
    SetBody {
        id: String,
        text: String,
    },
    RemoveElement {
        id: String,
    },
    /// Low-level: replace the bytes `old` found at byte offset `at` with `new`. Fails if
    /// the bytes there are not `old`. Every inverse is one of these, so undo restores the
    /// exact bytes. Offsets are UTF-8 byte offsets into `deck.text()`.
    Replace {
        at: usize,
        old: String,
        new: String,
    },
    /// All or nothing. The inverse is one `Batch`.
    Batch {
        ops: Vec<Op>,
    },
}

fn err(code: &'static str, message: impl Into<String>) -> EngineError {
    EngineError::new(code, message)
}

/// Element counts per container (masters, then slides), used to check structure.
fn shape(d: &Deck) -> Vec<usize> {
    d.masters
        .iter()
        .map(|m| m.elements.len())
        .chain(d.layouts.iter().map(|l| l.placeholders.len()))
        .chain(d.slides.iter().map(|s| s.elements.len()))
        .collect()
}

/// Which container (index into `shape`) holds element `id`, and its position there.
fn locate(d: &Deck, id: &str) -> Option<(usize, usize)> {
    let mut c = 0;
    for m in &d.masters {
        if let Some(i) = m.elements.iter().position(|e| e.id == id) {
            return Some((c, i));
        }
        c += 1;
    }
    c += d.layouts.len();
    for s in &d.slides {
        if let Some(i) = s.elements.iter().position(|e| e.id == id) {
            return Some((c, i));
        }
        c += 1;
    }
    None
}

fn element_at(d: &Deck, (c, i): (usize, usize)) -> Option<&Element> {
    let m = d.masters.len();
    let l = d.layouts.len();
    if c < m {
        d.masters[c].elements.get(i)
    } else if c >= m + l {
        d.slides.get(c - m - l)?.elements.get(i)
    } else {
        None
    }
}

/// What the re-read deck must look like for the edit to count.
enum Expect {
    Same {
        at: (usize, usize),
        id: String,
        body: Option<String>,
    },
    Removed {
        at: (usize, usize),
    },
    Anything,
}

struct Plan {
    range: Range<usize>,
    text: String,
    expect: Expect,
}

impl Deck {
    /// Apply one op. On error the deck is unchanged. Returns the inverse op.
    pub fn apply(&mut self, op: Op) -> Result<Op, EngineError> {
        if self.read_only {
            return Err(err("E-READ-ONLY", "This deck is read-only."));
        }
        if let Op::Batch { ops } = op {
            let backup = self.clone();
            let mut inverses = Vec::with_capacity(ops.len());
            for o in ops {
                match self.apply(o) {
                    Ok(inv) => inverses.push(inv),
                    Err(e) => {
                        *self = backup;
                        return Err(e);
                    }
                }
            }
            inverses.reverse();
            return Ok(Op::Batch { ops: inverses });
        }
        let plan = self.plan(&op)?;
        let old = self.source[plan.range.clone()].to_string();
        let mut next = String::with_capacity(self.source.len() + plan.text.len());
        next.push_str(&self.source[..plan.range.start]);
        next.push_str(&plan.text);
        next.push_str(&self.source[plan.range.end..]);

        // Keep in-memory ids of untouched elements: shift their anchors past the edit.
        let delta = plan.text.len() as isize - plan.range.len() as isize;
        let carry: Carry = self
            .carry()
            .into_iter()
            .filter_map(|(at, id)| {
                if at < plan.range.start {
                    Some((at, id))
                } else if at >= plan.range.end {
                    Some(((at as isize + delta) as usize, id))
                } else {
                    None
                }
            })
            .collect();
        let candidate = Deck::build(&next, &carry);
        let ok = match &plan.expect {
            Expect::Anything => true,
            Expect::Same { at, id, body } => {
                shape(&candidate) == shape(self)
                    && element_at(&candidate, *at)
                        .is_some_and(|e| &e.id == id && body.as_ref().is_none_or(|b| &e.body == b))
            }
            Expect::Removed { at } => {
                let mut want = shape(self);
                want[at.0] -= 1;
                shape(&candidate) == want
            }
        };
        if !ok {
            return Err(err(
                "E-STRUCTURE",
                "The edit would change the deck around the element (for example a body that adds --- or a directive line).",
            ));
        }
        let inverse = Op::Replace {
            at: plan.range.start,
            old: plan.text,
            new: old,
        };
        *self = candidate;
        Ok(inverse)
    }

    /// Apply several ops as one step.
    pub fn apply_batch(&mut self, ops: Vec<Op>) -> Result<Op, EngineError> {
        self.apply(Op::Batch { ops })
    }

    fn find(&self, id: &str) -> Result<((usize, usize), &Element), EngineError> {
        locate(self, id)
            .and_then(|at| element_at(self, at).map(|e| (at, e)))
            .ok_or_else(|| err("E-NOT-FOUND", format!("No element with id {id}.")).on(id))
    }

    fn eol(&self) -> &'static str {
        self.line_ending.as_str()
    }

    fn plan(&self, op: &Op) -> Result<Plan, EngineError> {
        match op {
            Op::SetGeometry {
                id,
                x,
                y,
                w,
                h,
                rot,
            } => {
                let (at, e) = self.find(id)?;
                let mut set = Vec::new();
                for (k, v) in [("x", x), ("y", y), ("w", w), ("h", h), ("rot", rot)] {
                    let Some(v) = v else { continue };
                    if !v.is_finite() {
                        return Err(err("E-ATTR-VALUE", format!("{k} must be a number.")).on(id));
                    }
                    if (k == "w" || k == "h") && *v < 0.0 {
                        return Err(
                            err("E-ATTR-VALUE", format!("{k} must not be negative.")).on(id)
                        );
                    }
                    set.push((k, fmt_num(*v)));
                }
                let (range, text) = if e.implicit {
                    // First edit of implicit text: write a directive line above it.
                    let line = self.implicit_directive(e, &set);
                    (
                        e.src.body.start..e.src.body.start,
                        format!("{line}{}", self.eol()),
                    )
                } else {
                    let line = e.src.line.clone().unwrap_or_default();
                    (line.clone(), self.edit_directive(e, &set))
                };
                Ok(Plan {
                    range,
                    text,
                    expect: Expect::Same {
                        at,
                        id: id.clone(),
                        body: None,
                    },
                })
            }
            Op::SetBody { id, text } => {
                let (at, e) = self.find(id)?;
                let mut body = text.clone();
                if self.eol() == "\r\n" && !body.contains('\r') {
                    body = body.replace('\n', "\r\n");
                }
                let at_eof = e.src.body.end == self.source.len();
                if !body.is_empty() && !body.ends_with('\n') && !at_eof {
                    body.push_str(self.eol());
                }
                let expect_body = body[..trim_blank_tail(&body, 0..body.len())].to_string();
                let (range, replacement) = if e.implicit {
                    let line = self.implicit_directive(e, &[]);
                    (e.src.body.clone(), format!("{line}{}{body}", self.eol()))
                } else {
                    let line = e.src.line.clone().unwrap_or_default();
                    let has_eol = e.src.body.start > line.end;
                    let between = if has_eol {
                        self.source[line.end..e.src.body.start].to_string()
                    } else if body.is_empty() {
                        String::new()
                    } else {
                        self.eol().to_string()
                    };
                    if e.id_written() {
                        let start = if has_eol { e.src.body.start } else { line.end };
                        (
                            start..e.src.body.end,
                            format!("{}{body}", if has_eol { "" } else { between.as_str() }),
                        )
                    } else {
                        let new_line = self.edit_directive(e, &[]);
                        (
                            line.start..e.src.body.end,
                            format!("{new_line}{between}{body}"),
                        )
                    }
                };
                Ok(Plan {
                    range,
                    text: replacement,
                    expect: Expect::Same {
                        at,
                        id: id.clone(),
                        body: Some(expect_body),
                    },
                })
            }
            Op::RemoveElement { id } => {
                let (at, e) = self.find(id)?;
                Ok(Plan {
                    range: e.src.start()..e.src.end,
                    text: String::new(),
                    expect: Expect::Removed { at },
                })
            }
            Op::Replace { at, old, new } => {
                let end = at.checked_add(old.len()).unwrap_or(usize::MAX);
                if self.source.get(*at..end) != Some(old.as_str()) {
                    return Err(err(
                        "E-STALE",
                        "The text at this offset has changed since the op was made.",
                    ));
                }
                Ok(Plan {
                    range: *at..end,
                    text: new.clone(),
                    expect: Expect::Anything,
                })
            }
            Op::Batch { .. } => Err(err("E-INTERNAL", "Batch is handled by apply.")),
        }
    }

    /// The directive line of `e` with `set` applied and its id written if needed.
    fn edit_directive(&self, e: &Element, set: &[(&str, String)]) -> String {
        let line = e.src.line.clone().unwrap_or_default();
        let base = line.start;
        let text = &self.source[line];
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        let mut appended = String::new();
        let mut set: Vec<(&str, String)> = set.to_vec();
        if !e.id_written() {
            set.insert(0, ("id", e.id.clone()));
        }
        for (key, value) in &set {
            match e.attrs.iter().rev().find(|a| a.key == *key) {
                Some(a) if a.value.as_deref() == Some(value.as_str()) => {}
                Some(a) => edits.push((
                    a.span.start - base..a.span.end - base,
                    format!("{key}={}", quote(value)),
                )),
                None => appended.push_str(&format!(" {key}={}", quote(value))),
            }
        }
        if !appended.is_empty() {
            let at = e.src.insert_at - base;
            edits.push((at..at, appended));
        }
        edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
        let mut out = text.to_string();
        for (r, t) in edits {
            out.replace_range(r, &t);
        }
        out
    }

    /// A directive line for implicit text, with its current geometry and `set` applied.
    fn implicit_directive(&self, e: &Element, set: &[(&str, String)]) -> String {
        let get = |k: &str, v: f32| {
            set.iter()
                .find(|(key, _)| *key == k)
                .map_or_else(|| fmt_num(v as f64), |(_, s)| s.clone())
        };
        let mut line = format!(
            "<!-- text id={} x={} y={} w={} h={}",
            e.id,
            get("x", e.x),
            get("y", e.y),
            get("w", e.w),
            get("h", e.h)
        );
        if let Some((_, r)) = set.iter().find(|(k, _)| *k == "rot") {
            line.push_str(&format!(" rot={r}"));
        }
        line.push_str(" -->");
        line
    }
}
