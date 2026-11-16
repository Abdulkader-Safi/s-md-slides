---
name: s-md-slides
description: Write and edit s-md-slides decks, then validate and export them. Decks are free-canvas slide decks stored in one markdown file, where each box is an HTML comment with its position followed by markdown. Use when asked to make slides, a deck, a presentation or a carousel as a .md file, to edit an existing s-md-slides deck, or to run the s-md-slides command.
---

# s-md-slides

A deck is one `.md` file. Each box on a slide is a one-line HTML comment giving its kind, id and position, followed by its content in normal markdown. Slides are separated by a line that is exactly `---`.

## The smallest useful deck

```markdown
---
s-md-slides: 1
title: Product update
canvas: 16:9
---

<!-- text id=t1 x=120 y=80 w=1680 h=200 size=72 weight=bold -->
# Product update

<!-- text id=t2 x=120 y=320 w=1680 h=640 size=40 -->
- Shipped the editor
- Fixed 42 bugs

<!-- notes -->
Speaker notes go here. They never show on the slide.

---

<!-- text id=t3 x=120 y=80 w=1680 h=200 size=72 weight=bold -->
# Next quarter
```

## Rules that keep a deck valid

1. Put front matter first, between `---` lines. Always set `s-md-slides: 1`.
2. Separate slides with a line that is exactly `---`. Never use `---` as a horizontal rule inside a slide: write `***` instead.
3. Every element needs `x`, `y`, `w` and `h` in canvas units (lines and arrows may skip `h`). Keep boxes inside the canvas: x + w no more than the width, y + h no more than the height.
4. Give every element a unique `id`: lowercase letters, digits and `-`, up to 32 characters. A short kind letter plus a number works well (`t1`, `i2`, `s3`).
5. A directive comment must be the whole line: `<!-- kind key=value ... -->`. Quote values with spaces: `name="Hero image"`.
6. Colors are `#rgb`, `#rrggbb` or a theme role: `background`, `text`, `accent1` to `accent6`, `code`, `code-bg`. Named colors like `red` are errors.
7. Every image needs alt text: `![What the image shows](assets/photo.jpg)`. Paths are relative to the deck file.
8. Notes come last on a slide, after `<!-- notes -->`.
9. `<!-- slide ... -->` goes on its own line, as the first line of the slide, before any element.

## Canvases

| `canvas:` | Size | Use for |
| --- | --- | --- |
| `16:9` (default) | 1920 x 1080 | presentations |
| `4:5` | 1080 x 1350 | social carousels |
| `WxH`, such as `1200x800` | any | custom |

A safe layout on 16:9: margins of 120, a title at `x=120 y=80 w=1680 h=200`, a body at `x=120 y=320 w=1680 h=640`. On 4:5: margins of 80, a title at `x=80 y=120 w=920 h=240`, a body at `x=80 y=420 w=920 h=800`.

## Element kinds

| Kind | Body | Key attributes |
| --- | --- | --- |
| `text` | markdown | `size`, `color`, `weight`, `align`, `valign`, `font`, `line-height`, `padding` |
| `image` | one `![alt](path)` | `fit=contain\|cover\|fill`, `crop=x,y,w,h` |
| `shape` | optional text (rect and ellipse) | `kind=rect\|ellipse\|line\|arrow`, `fill`, `stroke`, `stroke-width`, `radius` |
| `code` | one fenced block with a language | `lines=1,3-5` to highlight |
| `table` | one markdown table | `header=false` |
| `diagram` | one ` ```mermaid ` block | |
| `math` | `$$...$$` or a ` ```math ` block | |
| `video` | empty | `src`, `poster`, `autoplay`, `loop`, `muted` |
| `embed` | empty | `src` (https only), `poster` |
| `group` | empty | `items=id1,id2` |

Lines and arrows run from `(x, y)` to `(x + w, y + h)`, so `h=0` (or no `h`) is horizontal and `w=0` is vertical. An arrow has its head at the end; add `arrow-start=triangle` for a second head, or `arrow-end=none` to drop it.

Every kind also takes `rot`, `opacity`, `locked`, `name` and `dir`, plus animation: `enter` or `exit` (`appear`, `fade`, `fly-left`, `fly-right`, `fly-up`, `fly-down`, `scale`), `order` (which click plays it, from 1), `delay` and `dur` in milliseconds.

````markdown
<!-- shape id=s1 kind=rect x=1000 y=320 w=800 h=500 fill=accent2 radius=24 enter=scale order=2 -->
Every edit is one line in git

<!-- code id=c1 x=120 y=320 w=800 h=300 lines=2 -->
```python
print("hello")
print("deck")
```
````

## Slide settings, masters, themes and direction

`<!-- slide bg=#0b1020 transition=push dur=500 -->` as the first line of a slide sets its background (a color, `linear(135,accent1,accent5)` or an image path) and its transition (`none`, `fade`, `push`).

A master holds elements shown behind every slide, and its text can use `{slide}`, `{total}` and `{title}`. Put masters and layouts in a section of their own before the first slide. That section is not a slide, so the first slide after it is slide 1:

```markdown
---
title: Branded
---

<!-- master id=brand -->
<!-- text id=footer x=120 y=1010 w=600 h=40 size=20 -->
{title}: {slide} / {total}

---

# First slide
```

`<!-- slide master=none -->` turns the master off for one slide. A theme goes in front matter:

```markdown
---
theme:
  colors:
    accent1: "#f59e0b"
  fonts:
    heading: Inter
---

# Themed
```

For Arabic or other right-to-left decks, set `dir: rtl` and `lang: ar`. Positions are never mirrored; only text direction changes.

## The command

| Command | What it does |
| --- | --- |
| `s-md-slides validate deck.md` | Lists problems as `file:line:col: severity code message`. Exits 1 on errors. Add `--json` for tools. |
| `s-md-slides export deck.md --to html\|pdf\|png\|jpeg\|gif` | Writes next to the deck. `--slide N`, `--scale 2`, `--embed` (offline HTML), `-o path`. |
| `s-md-slides serve deck.md` | Live preview at http://127.0.0.1:4321 that reloads on save. Never writes. |
| `s-md-slides apply deck.md ops.json` | Applies ops, writes the deck, prints the undo. `--dry-run` writes nothing. |

After writing or editing a deck, always run `s-md-slides validate` and fix every error before you finish.

## Editing with ops

Prefer small edits through ops over rewriting the file: they change only the lines of the elements they touch, keep everything else byte for byte, and return their own undo. An op is JSON with an `"op"` field:

| Op | JSON |
| --- | --- |
| Move, resize, rotate | `{"op":"set_geometry","id":"t1","x":200,"y":80,"w":800,"rot":15}` |
| Change attributes | `{"op":"set_attrs","id":"t1","set":{"color":"accent1","size":48},"remove":["locked"]}` |
| Replace text | `{"op":"set_body","id":"t1","text":"# New title"}` |
| Add an element | `{"op":"add_element","slide":0,"kind":"text","attrs":{"x":120,"y":80,"w":800,"h":200},"body":"Hello"}` |
| Remove, duplicate | `{"op":"remove_element","id":"t1"}`, `{"op":"duplicate_element","id":"t1","dx":20,"dy":20}` |
| Stacking order | `{"op":"set_z","id":"t1","to":"front"}` (or `back`, `forward`, `backward`, or an index) |
| Slides | `add_slide` (`index`, optional `layout` or `raw`), `remove_slide`, `duplicate_slide`, `move_slide` (`from`, `to`) |
| Notes | `{"op":"set_notes","slide":0,"text":"Say this"}` (`null` removes them) |
| Slide settings | `{"op":"set_slide_attrs","slide":0,"set":{"bg":"#111","transition":"fade"}}` |
| Front matter | `{"op":"set_front_matter","key":"title","value":"New title"}` (`null` removes the key) |
| Masters and layouts | `add_definition` (`raw`), `remove_definition` (`id`) |
| Groups | `{"op":"group","ids":["a","b"]}`, `{"op":"ungroup","id":"g1"}` |
| Several at once | `{"op":"batch","ops":[...]}`: all or nothing, one undo |

Slides are numbered from 0 in ops. An op that would break the deck (an unknown id, a negative width, a body containing `---`) is refused with a code such as `E-NOT-FOUND` and changes nothing.

## Desktop app tools (MCP)

The desktop app exposes one MCP tool per op, named after it and taking the same JSON without the `op` field: `set_geometry`, `set_attrs`, `set_body`, `add_element`, `remove_element`, `duplicate_element`, `set_z`, `add_slide`, `remove_slide`, `duplicate_slide`, `move_slide`, `set_notes`, `set_slide_attrs`, `set_front_matter`, `add_definition`, `remove_definition`, `group`, `ungroup` and `batch`. Each returns the inverse op, which the app keeps as its undo. Read-only tools mirror the engine's queries: `deck` (the parsed model), `diagnostics`, `elements_at` (slide, x, y), `search` (text) and `assets`.

## Checklist before you finish

1. `s-md-slides: 1` and a `canvas` in front matter.
2. Slides separated by `---` lines, and no `---` inside a slide.
3. Every element has a unique id and `x y w h` inside the canvas.
4. Every image has alt text.
5. `s-md-slides validate deck.md` exits 0.
