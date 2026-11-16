// Browser check in Safari through safaridriver (WebDriver). Puppeteer can't drive Safari.
// Needs Safari > Settings > Advanced > "Show features for web developers", then
// Develop > "Allow Remote Automation". Run from engine/: node runtime/test/safari.mjs
// Same checks as check.mjs, except reduced motion (Safari can't emulate it over
// WebDriver) and console errors (WebDriver can't read Safari's console).

import { execFileSync, spawn } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const engine = new URL("../../", import.meta.url).pathname;
const axeSource = readFileSync(new URL("../node_modules/axe-core/axe.min.js", import.meta.url), "utf8");
const out = mkdtempSync(join(tmpdir(), "smd-safari-"));
function exportDeck(name) {
  const file = join(out, `${name}.html`);
  execFileSync("cargo", ["run", "-q", "-p", "s-md-slides", "--example", "export", "--", `examples/${name}.md`, file, "--embed"], {
    cwd: engine,
    stdio: ["ignore", "ignore", "inherit"],
  });
  return "file://" + file;
}
const pitch = exportDeck("pitch-16x9");
const carousel = exportDeck("carousel-4x5");

const port = 4724;
const driver = spawn("safaridriver", ["-p", String(port)], { stdio: "ignore" });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
await wait(1500);

const base = `http://localhost:${port}`;
async function call(method, path, body) {
  const r = await fetch(base + path, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const j = await r.json();
  if (j.value && j.value.error) throw new Error(`${path}: ${j.value.message}`);
  return j.value;
}

let failures = 0;
function check(ok, what) {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures++;
}

const session = await call("POST", "/session", { capabilities: { alwaysMatch: { browserName: "safari" } } });
const s = `/session/${session.sessionId}`;
console.log(`browser: Safari ${session.capabilities.browserVersion}`);
const run = (script, ...args) => call("POST", `${s}/execute/sync`, { script, args });
const keys = { ArrowRight: "", ArrowLeft: "", Space: " ", Home: "", End: "", Enter: "" };
async function press(name) {
  const k = keys[name];
  await call("POST", `${s}/actions`, {
    actions: [{ type: "key", id: "kb", actions: [{ type: "keyDown", value: k }, { type: "keyUp", value: k }] }],
  });
}
async function open(url) {
  await call("POST", `${s}/url`, { url: "about:blank" });
  await call("POST", `${s}/url`, { url });
  for (let i = 0; i < 50; i++) {
    if (await run("return !!window.smd && !document.body.classList.contains('smd-loading')")) return;
    await wait(100);
  }
  throw new Error("page did not load");
}
const state = () => run("return window.smd.state()");
const style = (id, prop) =>
  run(
    "const el = Array.from(document.querySelectorAll('section.smd-slide')).find(s => s.style.display !== 'none')?.querySelector('[data-id=\"' + arguments[0] + '\"]'); return el ? el.style[arguments[1]] : null",
    id,
    prop,
  );

try {
  await call("POST", `${s}/window/rect`, { width: 1280, height: 800 });
  await open(pitch + "#2");
  await wait(300);
  check((await state()).slide === 1, "opens at the slide in the URL hash");
  check((await run("return document.querySelector('#smd-counter').textContent")) === "2 / 4", "counter shows 2 / 4");
  check(((await style("b1", "transform")) || "").includes("translate(-"), "b1 waits off-canvas before its step");
  await press("ArrowRight");
  await wait(450);
  const mid = (await style("b1", "transform")) || "";
  check(mid.includes("translate(-"), `FR-ANIM-01: fly-left is still moving 450 ms in: ${mid}`);
  await wait(600);
  check((await style("b1", "transform")) === "", "fly-left ends at the element's place");
  check((await style("b2", "transform")) === "", "b2 plays in the same click as b1");
  check((await style("s1", "opacity")) === "0", "s1 waits for the second click");
  await press("Space");
  await wait(600);
  check((await style("s1", "opacity")) === "1", "second click shows s1");
  check((await state()).step === 2, "FR-ANIM-02: two clicks played");
  await press("ArrowRight");
  await wait(50);
  check((await state()).slide === 2, "a third click goes to the next slide");
  await press("ArrowRight");
  await press("ArrowRight");
  await wait(100);
  const pushed = await run("return Array.from(document.querySelectorAll('section.smd-slide')).map(s => s.style.transform).filter(Boolean)");
  check(pushed.some((t) => t.startsWith("translateX(")), `FR-ANIM-03: push moves the slides: ${pushed.join(" | ")}`);
  await wait(600);
  await press("End");
  await press("ArrowRight");
  await wait(50);
  check(await run("return !document.querySelector('#smd-end').hidden"), "end screen after the last slide");
  await press("Home");
  await wait(50);
  check((await state()).slide === 0, "Home goes to the first slide");
  check(
    await run("return [...document.images].every(i => i.src.startsWith('data:') && i.complete && i.naturalWidth > 0)"),
    "every image is embedded and decodes",
  );
  // axe-core on the controls (injected by the driver, so the page CSP doesn't apply).
  await run(axeSource);
  const v = await call("POST", `${s}/execute/async`, {
    script:
      "const done = arguments[arguments.length - 1]; axe.run({ include: [['.smd-controls'], ['#smd-progress']] }, { runOnly: { type: 'tag', values: ['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa'] } }).then(r => done(r.violations.map(x => x.id)))",
    args: [],
  });
  check(v.length === 0, `axe WCAG 2.2 AA on the controls: ${v.join(", ") || "no violations"}`);

  await open(carousel);
  const left = await style("t1", "left");
  await press("ArrowLeft");
  await wait(50);
  await press("ArrowLeft");
  await wait(50);
  check((await state()).slide === 1, "rtl: Left Arrow goes to the next slide");
  await press("ArrowRight");
  await wait(50);
  check((await state()).slide === 0, "rtl: Right Arrow goes back");
  check((await style("t1", "left")) === left && left === "80px", "rtl: elements keep their x");

  await open(pitch + "#presenter");
  await wait(300);
  check((await run("return document.querySelector('.smd-pv-notes').textContent")).includes("Open with the problem"), "presenter view shows the notes");
  check(await run("return document.querySelector('.smd-pv-next section')?.dataset.slide === '1'"), "presenter view shows the next slide");
  check(!(await run("return document.querySelector('#smd-stage').textContent")).includes("Open with the problem"), "FR-NOTES-01: notes never on the slide");
} catch (e) {
  check(false, String(e));
} finally {
  await call("DELETE", s).catch(() => {});
  driver.kill();
}
console.log(failures ? `${failures} check(s) failed` : "all checks passed");
process.exit(failures ? 1 : 0);
