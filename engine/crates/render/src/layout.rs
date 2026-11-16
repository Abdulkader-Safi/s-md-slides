//! Markdown text to positioned glyph runs with parley. Coordinates are local to the
//! element box (0,0 is its top-left corner).

use crate::fonts::{Brush, FALLBACK, Inner, MONO_FALLBACK};
use parley::{
    Alignment, AlignmentOptions, BaseDirection, FontFamily, FontStyle, FontWeight, Layout,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use pulldown_cmark::{Alignment as ColAlign, Event, Options, Parser, Tag, TagEnd};
use s_md_slides::{Dir, ResolvedStyle};
use std::ops::Range;
use std::sync::Arc;

/// One glyph: its id, its offset within the run and its advance, plus the source text
/// it stands for (ligature continuations merged).
#[derive(Debug, Clone)]
pub(crate) struct Glyph {
    pub id: u32,
    pub dx: f32,
    pub dy: f32,
    pub advance: f32,
    pub range: Range<usize>,
}

/// Glyphs in one font, size and color on one line.
#[derive(Debug, Clone)]
pub(crate) struct Run {
    pub font: parley::FontData,
    pub size: f32,
    pub color: Brush,
    /// Start of the run on its baseline, in element coordinates.
    pub x: f64,
    pub y: f64,
    pub glyphs: Vec<Glyph>,
    /// Synthetic italic (skew angle in degrees) and bold, when the font lacks them.
    pub skew: f32,
    pub embolden: bool,
    pub text: Arc<str>,
}

/// Laid-out text: glyph runs plus extra SVG drawn behind them (table rules, quote bars,
/// code line highlights).
#[derive(Debug, Default)]
pub(crate) struct TextBox {
    pub runs: Vec<Run>,
    pub svg: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Span {
    bold: bool,
    italic: bool,
    code: bool,
    link: bool,
}

#[derive(Debug, Clone)]
struct Para {
    text: String,
    spans: Vec<(Range<usize>, Span)>,
    heading: bool,
    code_block: bool,
    indent: f64,
    quote: bool,
}

#[derive(Debug, Clone)]
enum Block {
    Para(Para),
    Rule,
    Table {
        align: Vec<ColAlign>,
        rows: Vec<Vec<Para>>,
        header_rows: usize,
    },
}

fn empty_para() -> Para {
    Para {
        text: String::new(),
        spans: Vec::new(),
        heading: false,
        code_block: false,
        indent: 0.0,
        quote: false,
    }
}

/// Markdown to blocks. Raw HTML is dropped (native export can't draw it).
fn blocks(md: &str) -> Vec<Block> {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    let mut out = Vec::new();
    let mut para: Option<Para> = None;
    let mut span = Span::default();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut quote = 0usize;
    let mut table: Option<(Vec<ColAlign>, Vec<Vec<Para>>, usize)> = None;
    let mut in_head = false;
    let indent = |lists: &Vec<Option<u64>>, quote: usize| {
        (lists.len().saturating_sub(1) as f64) * 1.4 + quote as f64 * 1.0
    };
    let push_text = |para: &mut Option<Para>, s: &str, span: Span| {
        let p = para.get_or_insert_with(empty_para);
        let start = p.text.len();
        p.text.push_str(s);
        if span != Span::default() {
            p.spans.push((start..p.text.len(), span));
        }
    };
    for ev in Parser::new_ext(md, opts) {
        match ev {
            Event::Start(Tag::Paragraph) => {
                if para.as_ref().is_some_and(|p| !p.text.is_empty()) && table.is_none() {
                    // A paragraph inside a list item continues on its own line.
                    if let Some(p) = para.take() {
                        out.push(Block::Para(p));
                    }
                }
                let p = para.get_or_insert_with(empty_para);
                p.indent = indent(&lists, quote) + if lists.is_empty() { 0.0 } else { 1.4 };
                p.quote = quote > 0;
            }
            Event::Start(Tag::Heading { .. }) => {
                para = Some(Para {
                    heading: true,
                    indent: indent(&lists, quote),
                    quote: quote > 0,
                    ..empty_para()
                });
            }
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::List(start)) => {
                if let Some(p) = para.take().filter(|p| !p.text.is_empty()) {
                    out.push(Block::Para(p));
                }
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                if let Some(p) = para.take().filter(|p| !p.text.is_empty()) {
                    out.push(Block::Para(p));
                }
                let marker = match lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "\u{2022} ".to_string(),
                };
                let mut p = Para {
                    indent: indent(&lists, quote),
                    quote: quote > 0,
                    ..empty_para()
                };
                p.text.push_str(&marker);
                para = Some(p);
            }
            Event::TaskListMarker(done) => push_text(
                &mut para,
                if done { "\u{2611} " } else { "\u{2610} " },
                span,
            ),
            Event::Start(Tag::CodeBlock(_)) => {
                if let Some(p) = para.take().filter(|p| !p.text.is_empty()) {
                    out.push(Block::Para(p));
                }
                para = Some(Para {
                    code_block: true,
                    indent: indent(&lists, quote),
                    quote: quote > 0,
                    ..empty_para()
                });
            }
            Event::Start(Tag::Table(align)) => table = Some((align, Vec::new(), 0)),
            Event::Start(Tag::TableHead) => {
                in_head = true;
                if let Some(t) = &mut table {
                    t.1.push(Vec::new());
                }
            }
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                if let Some(t) = &mut table {
                    t.2 = 1;
                }
            }
            Event::Start(Tag::TableRow) => {
                if let Some(t) = &mut table {
                    t.1.push(Vec::new());
                }
            }
            Event::Start(Tag::TableCell) => para = Some(empty_para()),
            Event::End(TagEnd::TableCell) => {
                let mut p = para.take().unwrap_or_else(empty_para);
                p.heading = in_head;
                if let Some(row) = table.as_mut().and_then(|t| t.1.last_mut()) {
                    row.push(p);
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some((align, rows, head)) = table.take() {
                    out.push(Block::Table {
                        align,
                        rows,
                        header_rows: head,
                    });
                }
            }
            Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::CodeBlock,
            ) => {
                if table.is_none()
                    && let Some(mut p) = para.take().filter(|p| !p.text.trim().is_empty())
                {
                    if p.code_block && p.text.ends_with('\n') {
                        p.text.pop();
                    }
                    out.push(Block::Para(p));
                }
            }
            Event::Start(Tag::Strong) => span.bold = true,
            Event::End(TagEnd::Strong) => span.bold = false,
            Event::Start(Tag::Emphasis) => span.italic = true,
            Event::End(TagEnd::Emphasis) => span.italic = false,
            Event::Start(Tag::Link { .. }) => span.link = true,
            Event::End(TagEnd::Link) => span.link = false,
            Event::Text(t) => {
                let code = para.as_ref().is_some_and(|p| p.code_block);
                push_text(
                    &mut para,
                    &t,
                    Span {
                        code: code || span.code,
                        ..span
                    },
                );
            }
            Event::Code(t) => push_text(&mut para, &t, Span { code: true, ..span }),
            Event::InlineMath(m) => push_text(&mut para, &format!("${m}$"), span),
            Event::DisplayMath(m) => push_text(&mut para, &format!("$${m}$$"), span),
            Event::SoftBreak => push_text(&mut para, " ", span),
            Event::HardBreak => push_text(&mut para, "\n", span),
            Event::Rule => out.push(Block::Rule),
            _ => {}
        }
    }
    if let Some(p) = para.filter(|p| !p.text.trim().is_empty()) {
        out.push(Block::Para(p));
    }
    out
}

/// Text settings for one box.
pub(crate) struct TextOpts<'a> {
    pub style: &'a ResolvedStyle,
    pub heading_font: &'a str,
    pub code_font: &'a str,
    pub link: Brush,
    pub dir: Dir,
    /// Force an alignment (shape text is centered).
    pub align: Option<&'a str>,
    pub valign: Option<&'a str>,
}

pub(crate) fn rgba(hex: &str) -> Brush {
    let h = hex.trim_start_matches('#');
    let p = |i: usize, n: usize| -> u8 {
        let s = &h[i..i + n];
        let v = u8::from_str_radix(s, 16).unwrap_or(0);
        if n == 1 { v * 17 } else { v }
    };
    match h.len() {
        3 => [p(0, 1), p(1, 1), p(2, 1), 255],
        4 => [p(0, 1), p(1, 1), p(2, 1), p(3, 1)],
        6 => [p(0, 2), p(2, 2), p(4, 2), 255],
        8 => [p(0, 2), p(2, 2), p(4, 2), p(6, 2)],
        _ => [0, 0, 0, 255],
    }
}

fn stack(font: &str, mono: bool) -> String {
    format!("'{font}', {}", if mono { MONO_FALLBACK } else { FALLBACK })
}

/// Lay out one paragraph at `width`. Returns the layout and its text.
fn layout_para(
    f: &mut Inner,
    p: &Para,
    o: &TextOpts,
    size: f32,
    color: Brush,
    width: Option<f32>,
) -> (Layout<Brush>, Arc<str>) {
    let st = o.style;
    let text: Arc<str> = Arc::from(p.text.as_str());
    let mono = p.code_block;
    let (font, lh) = if p.heading {
        (o.heading_font, 1.15)
    } else if mono {
        (o.code_font, st.line_height as f32)
    } else {
        (st.font.as_str(), st.line_height as f32)
    };
    let fam = stack(font, mono);
    let code_fam = stack(o.code_font, true);
    let Inner { fcx, lcx, .. } = f;
    let mut b = lcx.ranged_builder(fcx, &text, 1.0, true);
    b.push_default(StyleProperty::FontFamily(FontFamily::Source(
        fam.as_str().into(),
    )));
    b.push_default(StyleProperty::FontSize(if mono {
        size * 0.9
    } else {
        size
    }));
    b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(lh)));
    b.push_default(StyleProperty::Brush(color));
    let weight = if p.heading { 700.0 } else { st.weight as f32 };
    b.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
    for (r, s) in &p.spans {
        if s.bold {
            b.push(StyleProperty::FontWeight(FontWeight::new(700.0)), r.clone());
        }
        if s.italic {
            b.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
        }
        if s.code && !mono {
            b.push(
                StyleProperty::FontFamily(FontFamily::Source(code_fam.as_str().into())),
                r.clone(),
            );
            b.push(StyleProperty::FontSize(size * 0.9), r.clone());
        }
        if s.link {
            b.push(StyleProperty::Brush(o.link), r.clone());
        }
    }
    b.set_base_direction(match o.dir {
        Dir::Rtl => BaseDirection::Rtl,
        Dir::Ltr => BaseDirection::Ltr,
        Dir::Auto => BaseDirection::Auto,
    });
    let mut layout: Layout<Brush> = b.build(&text);
    // Alignment applies when lines break, so set it first.
    let align = match o.align.unwrap_or(st.align.as_str()) {
        "center" => Alignment::Center,
        "right" => Alignment::Right,
        "left" => Alignment::Left,
        "justify" => Alignment::Justify,
        "end" => Alignment::End,
        _ => Alignment::Start,
    };
    layout.set_alignment(align, AlignmentOptions::default());
    layout.break_all_lines(width);
    (layout, text)
}

/// Turn a parley layout into runs placed at (`x0`, `y0`).
fn collect(layout: &Layout<Brush>, text: &Arc<str>, x0: f64, y0: f64, out: &mut Vec<Run>) {
    for line in layout.lines() {
        let lr = line.text_range();
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(gr) = item else {
                continue;
            };
            let run = gr.run();
            // Ligatures: the start cluster holds the glyph; merge continuation ranges.
            let mut clusters: Vec<(Range<usize>, Vec<parley::Glyph>)> = run
                .visual_clusters()
                .map(|c| (c.text_range(), c.glyphs().collect::<Vec<_>>()))
                .filter(|(r, _)| r.start >= lr.start && r.end <= lr.end)
                .collect();
            clusters.sort_by_key(|(r, _)| r.start);
            let mut merged: Vec<(Range<usize>, Vec<parley::Glyph>)> = Vec::new();
            for (r, g) in clusters {
                match merged.last_mut() {
                    Some(last) if g.is_empty() => last.0.end = r.end,
                    _ => merged.push((r, g)),
                }
            }
            if run.is_rtl() {
                merged.reverse();
            }
            let glyphs: Vec<Glyph> = merged
                .into_iter()
                .flat_map(|(r, gs)| {
                    gs.into_iter().map(move |g| Glyph {
                        id: g.id,
                        dx: g.x,
                        dy: g.y,
                        advance: g.advance,
                        range: r.clone(),
                    })
                })
                .collect();
            if glyphs.is_empty() {
                continue;
            }
            let synth = run.synthesis();
            out.push(Run {
                font: run.font().clone(),
                size: run.font_size(),
                color: gr.style().brush,
                x: x0 + gr.offset() as f64,
                y: y0 + gr.baseline() as f64,
                glyphs,
                skew: synth.skew().unwrap_or(0.0),
                embolden: synth.embolden(),
                text: text.clone(),
            });
        }
    }
}

/// Lay out a markdown body inside a `w` by `h` box.
pub(crate) fn text_box(f: &mut Inner, md: &str, o: &TextOpts, w: f64, h: f64) -> TextBox {
    let st = o.style;
    let [pt, pr, pb, pl] = st.padding;
    let inner_w = (w - pl - pr).max(1.0);
    let size = st.size as f32;
    let color = rgba(&st.color);
    let mut tb = TextBox::default();
    let mut y = 0.0f64;
    let gap = |heading: bool| if heading { 0.4 } else { 0.6 } * size as f64;
    let blocks = blocks(md);
    let n = blocks.len();
    let mut placed: Vec<Run> = Vec::new();
    let mut extra = String::new();
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Para(p) => {
                let indent = p.indent * size as f64;
                let width = if p.code_block {
                    None
                } else {
                    Some((inner_w - indent).max(1.0) as f32)
                };
                let (layout, text) = layout_para(f, p, o, size, color, width);
                if p.quote {
                    extra.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" />",
                        pl + indent - 0.8 * size as f64,
                        pt + y,
                        0.2 * size as f64,
                        layout.height(),
                        hex(o.link)
                    ));
                }
                collect(&layout, &text, pl + indent, pt + y, &mut placed);
                y += layout.height() as f64;
                if i + 1 < n {
                    y += gap(p.heading);
                }
            }
            Block::Rule => {
                extra.push_str(&format!(
                    "<rect x=\"{pl}\" y=\"{}\" width=\"{inner_w}\" height=\"2\" fill=\"{}\" fill-opacity=\"0.3\" />",
                    pt + y + 0.3 * size as f64,
                    hex(color)
                ));
                y += 0.6 * size as f64 + if i + 1 < n { gap(false) } else { 0.0 };
            }
            Block::Table {
                align,
                rows,
                header_rows,
            } => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
                let col_w = inner_w / cols as f64;
                let (padx, pady) = (0.6 * size as f64, 0.35 * size as f64);
                for (ri, row) in rows.iter().enumerate() {
                    let mut row_h = 0.0f64;
                    let mut cells = Vec::new();
                    for (ci, cell) in row.iter().enumerate() {
                        let mut cell = cell.clone();
                        cell.heading = false;
                        let a = match align.get(ci) {
                            Some(ColAlign::Center) => "center",
                            Some(ColAlign::Right) => "right",
                            Some(ColAlign::Left) => "left",
                            _ => "start",
                        };
                        let bold = ri < *header_rows && st.weight < 700;
                        if bold {
                            cell.spans.push((
                                0..cell.text.len(),
                                Span {
                                    bold: true,
                                    ..Span::default()
                                },
                            ));
                        }
                        let co = TextOpts {
                            align: Some(a),
                            ..*o
                        };
                        let (layout, text) = layout_para(
                            f,
                            &cell,
                            &co,
                            size,
                            color,
                            Some((col_w - 2.0 * padx).max(1.0) as f32),
                        );
                        row_h = row_h.max(layout.height() as f64);
                        cells.push((ci, layout, text));
                    }
                    for (ci, layout, text) in cells {
                        collect(
                            &layout,
                            &text,
                            pl + ci as f64 * col_w + padx,
                            pt + y + pady,
                            &mut placed,
                        );
                    }
                    y += row_h + 2.0 * pady;
                    let thick = if ri + 1 == *header_rows { 2.0 } else { 1.0 };
                    extra.push_str(&format!(
                        "<rect x=\"{pl}\" y=\"{}\" width=\"{inner_w}\" height=\"{thick}\" fill=\"{}\" fill-opacity=\"0.25\" />",
                        pt + y - thick,
                        hex(color)
                    ));
                }
                if i + 1 < n {
                    y += gap(false);
                }
            }
        }
    }
    // Vertical alignment inside the padded box.
    let inner_h = h - pt - pb;
    let shift = match o.valign.unwrap_or(st.valign.as_str()) {
        "middle" => ((inner_h - y) / 2.0).max(0.0),
        "bottom" => (inner_h - y).max(0.0),
        _ => 0.0,
    };
    for r in &mut placed {
        r.y += shift;
    }
    tb.runs = placed;
    tb.svg = if shift != 0.0 && !extra.is_empty() {
        format!("<g transform=\"translate(0 {shift})\">{extra}</g>")
    } else {
        extra
    };
    tb
}

/// Code, one line per row with no wrapping. Highlighted lines get a tinted band.
pub(crate) fn code_box(
    f: &mut Inner,
    code: &str,
    lines: &[[u32; 2]],
    o: &TextOpts,
    w: f64,
) -> TextBox {
    let st = o.style;
    let [pt, _pr, _pb, pl] = st.padding;
    let size = st.size as f32;
    let color = rgba(&st.color);
    let mut tb = TextBox::default();
    let mut y = pt;
    for (i, line) in code.trim_end_matches('\n').split('\n').enumerate() {
        let n = i as u32 + 1;
        let p = Para {
            text: if line.is_empty() {
                " ".into()
            } else {
                line.to_string()
            },
            code_block: false,
            ..empty_para()
        };
        let co = TextOpts {
            style: o.style,
            align: Some("left"),
            dir: Dir::Ltr,
            ..*o
        };
        let mut style = st.clone();
        style.font = o.code_font.to_string();
        let co = TextOpts {
            style: &style,
            ..co
        };
        let (layout, text) = layout_para(f, &p, &co, size, color, None);
        let lh = layout.height() as f64;
        if lines.iter().any(|[a, b]| (*a..=*b).contains(&n)) {
            tb.svg.push_str(&format!(
                "<rect x=\"0\" y=\"{y}\" width=\"{w}\" height=\"{lh}\" fill=\"{}\" fill-opacity=\"0.18\" />",
                hex(o.link)
            ));
        }
        collect(&layout, &text, pl, y, &mut tb.runs);
        y += lh;
    }
    tb
}

pub(crate) fn hex(c: Brush) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
