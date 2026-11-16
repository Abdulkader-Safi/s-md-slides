//! Animation timeline (FR-ANIM-01, FR-ANIM-02, FR-ANIM-04).
//!
//! One source of truth: every entry carries its from and to state, and both this
//! module and the browser runtime blend them with the same ease-out cubic curve. The
//! HTML presentation, the GIF export and every host therefore play the same thing.

use crate::deck::{Deck, Element};
use serde::Serialize;
use std::collections::BTreeMap;

/// How an element looks at one moment, relative to its place on the slide.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct State {
    pub opacity: f64,
    pub dx: f64,
    pub dy: f64,
    pub scale: f64,
}

impl State {
    pub const VISIBLE: State = State {
        opacity: 1.0,
        dx: 0.0,
        dy: 0.0,
        scale: 1.0,
    };

    fn lerp(a: State, b: State, p: f64) -> State {
        let m = |x: f64, y: f64| x + (y - x) * p;
        State {
            opacity: m(a.opacity, b.opacity),
            dx: m(a.dx, b.dx),
            dy: m(a.dy, b.dy),
            scale: m(a.scale, b.scale),
        }
    }
}

/// Ease-out cubic. The runtime uses the same formula.
pub fn ease(p: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    1.0 - (1.0 - p).powi(3)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Enter,
    Exit,
}

/// One element's animation within a step.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Entry {
    pub id: String,
    pub phase: Phase,
    pub effect: String,
    /// Milliseconds after the step starts.
    pub delay: u32,
    /// Milliseconds; 0 for `appear`.
    pub dur: u32,
    pub from: State,
    pub to: State,
}

/// Step 0 plays when the slide appears. Each later step is one click.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Step {
    /// Milliseconds from the start of the timeline.
    pub start: u32,
    pub duration: u32,
    pub entries: Vec<Entry>,
}

/// A slide's animation: its steps played back to back.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Timeline {
    /// State of animated elements before anything plays. Other elements are visible.
    pub initial: BTreeMap<String, State>,
    pub steps: Vec<Step>,
    /// Total milliseconds.
    pub duration: u32,
}

/// Where an effect starts (for an entrance) or ends (for an exit).
fn away(effect: &str, e: &Element, w: f64, h: f64) -> State {
    let hidden = State {
        opacity: 0.0,
        ..State::VISIBLE
    };
    match effect {
        "fly-left" => State {
            dx: -(e.x + e.w),
            ..State::VISIBLE
        },
        "fly-right" => State {
            dx: w - e.x,
            ..State::VISIBLE
        },
        "fly-up" => State {
            dy: h - e.y,
            ..State::VISIBLE
        },
        "fly-down" => State {
            dy: -(e.y + e.h),
            ..State::VISIBLE
        },
        "scale" => State {
            scale: 0.5,
            ..hidden
        },
        _ => hidden,
    }
}

impl Timeline {
    /// Every animated element's state at `t` milliseconds (clamped to the timeline).
    /// Elements that never animate are not listed; they are always visible.
    pub fn state_at(&self, t: f64) -> BTreeMap<String, State> {
        let mut out = self.initial.clone();
        for step in &self.steps {
            for e in &step.entries {
                let begin = (step.start + e.delay) as f64;
                if t < begin {
                    continue;
                }
                let p = if e.dur == 0 {
                    1.0
                } else {
                    (t - begin) / e.dur as f64
                };
                out.insert(e.id.clone(), State::lerp(e.from, e.to, ease(p)));
            }
        }
        out
    }

    /// States while step `k` plays, `t` milliseconds into it: earlier steps finished,
    /// later ones not started. Unlike `state_at`, this keeps zero-length steps (such as
    /// `appear`) apart, which a GIF or any click-free playback needs.
    pub fn state_at_step(&self, k: usize, t: f64) -> BTreeMap<String, State> {
        let mut out = self.initial.clone();
        for (i, step) in self.steps.iter().enumerate().take(k + 1) {
            for e in &step.entries {
                if i < k {
                    out.insert(e.id.clone(), e.to);
                    continue;
                }
                let begin = e.delay as f64;
                if t < begin {
                    continue;
                }
                let p = if e.dur == 0 {
                    1.0
                } else {
                    (t - begin) / e.dur as f64
                };
                out.insert(e.id.clone(), State::lerp(e.from, e.to, ease(p)));
            }
        }
        out
    }

    /// Milliseconds at which step `k` has finished playing.
    pub fn end_of_step(&self, k: usize) -> u32 {
        self.steps
            .get(k)
            .map_or(self.duration, |s| s.start + s.duration)
    }

    /// Clicks needed to show the whole slide (steps after step 0).
    pub fn clicks(&self) -> usize {
        self.steps.len().saturating_sub(1)
    }
}

impl Deck {
    /// The animation timeline of slide `index` (FR-ANIM-04).
    pub fn timeline(&self, index: usize) -> Option<Timeline> {
        let slide = self.slides.get(index)?;
        let (w, h) = (
            self.front.canvas.width as f64,
            self.front.canvas.height as f64,
        );
        // Step keys: an entrance at `order` (0 without one), an exit right after it.
        // Doubling the order leaves room for the exit at order * 2 + 1.
        let mut keyed: Vec<(u32, Entry)> = Vec::new();
        let mut initial = BTreeMap::new();
        for e in &slide.elements {
            let a = &e.anim;
            let order = a.order.unwrap_or(0) * 2;
            if let Some(effect) = &a.enter {
                let from = away(effect, e, w, h);
                initial.insert(e.id.clone(), from);
                keyed.push((order, entry(e, effect, Phase::Enter, from, State::VISIBLE)));
            }
            if let Some(effect) = &a.exit {
                let key = if a.enter.is_some() { order + 1 } else { order };
                keyed.push((
                    key,
                    entry(
                        e,
                        effect,
                        Phase::Exit,
                        State::VISIBLE,
                        away(effect, e, w, h),
                    ),
                ));
            }
        }
        let mut keys: Vec<u32> = keyed.iter().map(|(k, _)| *k).filter(|k| *k > 0).collect();
        keys.sort_unstable();
        keys.dedup();
        keys.insert(0, 0);
        let mut steps = Vec::with_capacity(keys.len());
        let mut start = 0;
        for k in keys {
            let entries: Vec<Entry> = keyed
                .iter()
                .filter(|(key, _)| *key == k)
                .map(|(_, e)| e.clone())
                .collect();
            let duration = entries.iter().map(|e| e.delay + e.dur).max().unwrap_or(0);
            steps.push(Step {
                start,
                duration,
                entries,
            });
            start += duration;
        }
        Some(Timeline {
            initial,
            steps,
            duration: start,
        })
    }
}

fn entry(e: &Element, effect: &str, phase: Phase, from: State, to: State) -> Entry {
    Entry {
        id: e.id.clone(),
        phase,
        effect: effect.to_string(),
        delay: e.anim.delay,
        dur: if effect == "appear" { 0 } else { e.anim.dur },
        from,
        to,
    }
}
