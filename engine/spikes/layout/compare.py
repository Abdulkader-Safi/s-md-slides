"""M0 spike: render slide.json in headless Chrome with the same fonts, read its line
breaks, and compare them with parley's (lines.json from the Rust spike).

Usage: python3 compare.py <out-dir>   (run the Rust spike first)
"""
import base64, json, pathlib, subprocess, sys, re, html

here = pathlib.Path(__file__).parent
out = pathlib.Path(sys.argv[1]).resolve()
slide = json.loads((here / "slide.json").read_text())
chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

def face(name, file):
    data = base64.b64encode((here / "fonts" / file).read_bytes()).decode()
    return f"@font-face{{font-family:'{name}';src:url(data:font/ttf;base64,{data})}}"

boxes = "".join(
    f'<div class="b" id="{b["id"]}" dir="{b["dir"]}" style="left:{b["x"]}px;top:{b["y"]}px;width:{b["w"]}px">{html.escape(b["text"])}</div>'
    for b in slide["boxes"]
)
page = f"""<!doctype html><meta charset="utf-8"><style>
{face("Noto Sans", "NotoSans-Regular.ttf")}
{face("Noto Sans Arabic", "NotoSansArabic-Regular.ttf")}
html,body{{margin:0;width:{slide["width"]}px;height:{slide["height"]}px;background:#fff}}
.b{{position:absolute;font-family:'Noto Sans','Noto Sans Arabic';font-size:{slide["font_size"]}px;
line-height:{slide["line_height"]};color:#111;text-align:start;white-space:normal}}
</style><body>{boxes}<pre id="out"></pre><script>
document.fonts.ready.then(() => {{
  const res = {{}};
  for (const el of document.querySelectorAll('.b')) {{
    const node = el.firstChild, text = node.data, lines = [];
    let top = null, cur = '';
    for (let i = 0; i < text.length; i++) {{
      const r = document.createRange(); r.setStart(node, i); r.setEnd(node, i + 1);
      const rects = r.getClientRects();
      if (!rects.length) {{ cur += text[i]; continue; }}
      const t = Math.round(rects[0].top);
      if (top !== null && Math.abs(t - top) > 10 && text[i].trim()) {{ lines.push(cur.trim()); cur = ''; }}
      if (top === null || (Math.abs(t - top) > 10 && text[i].trim())) top = t;
      cur += text[i];
    }}
    lines.push(cur.trim());
    res[el.id] = lines;
  }}
  document.getElementById('out').textContent = JSON.stringify(res);
  document.getElementById('out').style.display = 'none';
}});
</script>"""
(out / "slide.html").write_text(page)
url = (out / "slide.html").as_uri()
flags = ["--headless=new", "--disable-gpu", "--hide-scrollbars", "--virtual-time-budget=5000"]
dom = subprocess.run([chrome, *flags, "--dump-dom", url], capture_output=True, text=True).stdout
browser = json.loads(html.unescape(re.search(r'<pre id="out"[^>]*>(.*?)</pre>', dom, re.S).group(1)))
subprocess.run([chrome, *flags, f"--window-size={slide['width']},{slide['height']}",
                f"--screenshot={out / 'browser.png'}", url], capture_output=True)
native = json.loads((out / "lines.json").read_text())

ok = True
for b in slide["boxes"]:
    n, w = native[b["id"]], browser[b["id"]]
    same = n == w
    print(f"{b['id']}: {'match' if same else 'DIFFER'} ({len(n)} native lines, {len(w)} browser lines)")
    if not same:
        ok = False
        for i in range(max(len(n), len(w))):
            print(f"  native  {i}: {n[i] if i < len(n) else ''}")
            print(f"  browser {i}: {w[i] if i < len(w) else ''}")
sys.exit(0 if ok else 1)
