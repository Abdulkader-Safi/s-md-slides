// Browser check for exported presentations (M4 exit criteria), in headless Chrome.
// Run from engine/: node runtime/test/check.mjs  (CHROME_PATH overrides the browser)
// It exports the example decks with embedded assets into a temp folder that has no
// assets next to it, so a pass also proves the files work offline (FR-ASSET-02).

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, copyFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import puppeteer from "puppeteer-core";

const engine = new URL("../../", import.meta.url).pathname;
const chrome =
  process.env.CHROME_PATH ||
  (process.platform === "darwin"
    ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    : "/usr/bin/google-chrome");
const axeSource = readFileSync(new URL("../node_modules/axe-core/axe.min.js", import.meta.url), "utf8");

const out = mkdtempSync(join(tmpdir(), "smd-check-"));
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

let failures = 0;
function check(ok, what) {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures++;
}
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const browser = await puppeteer.launch({ executablePath: chrome, headless: true, args: ["--allow-file-access-from-files"] });

async function open(url, opts = {}) {
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 720 });
  const problems = [];
  page.on("console", (m) => m.type() === "error" && problems.push(m.text()));
  page.on("pageerror", (e) => problems.push(String(e)));
  page.on("requestfailed", (r) => problems.push(`request failed: ${r.url().slice(0, 80)}`));
  if (opts.reduced) await page.emulateMediaFeatures([{ name: "prefers-reduced-motion", value: "reduce" }]);
  if (opts.bypassCSP) await page.setBypassCSP(true);
  await page.goto(url, { waitUntil: "load" });
  await page.waitForFunction(() => !document.body.classList.contains("smd-loading"));
  return { page, problems };
}
const state = (page) => page.evaluate(() => window.smd.state());
const style = (page, id, prop) =>
  page.evaluate((id, prop) => document.querySelector(`section[style*="display"]:not([style*="display: none"]) [data-id="${id}"], section:not([style*="none"]) [data-id="${id}"]`)?.style[prop] ?? null, id, prop);

// Pitch deck: runs under its own CSP with no errors.
{
  const { page, problems } = await open(pitch + "#2");
  await wait(300);
  check((await state(page)).slide === 1, "opens at the slide in the URL hash");
  check((await page.$eval("#smd-counter", (e) => e.textContent)) === "2 / 4", "counter shows 2 / 4");
  check(await page.$eval("#smd-progress", (e) => e.getAttribute("aria-valuenow") === "2"), "progress bar reports slide 2");
  // FR-ANIM-02: orders 1, 1, 2 take two clicks.
  check((await style(page, "b1", "transform")).includes("translate(-"), "b1 waits off-canvas before its step");
  await page.keyboard.press("ArrowRight");
  await wait(450);
  const mid = await style(page, "b1", "transform");
  check(mid.includes("translate(-") , `FR-ANIM-01: fly-left is still moving 450 ms in (delay 300, dur 500): ${mid}`);
  await wait(500);
  check((await style(page, "b1", "transform")) === "", "fly-left ends at the element's place");
  check((await style(page, "b2", "transform")) === "", "b2 plays in the same click as b1");
  check((await style(page, "s1", "opacity")) === "0", "s1 waits for the second click");
  await page.keyboard.press("Space");
  await wait(600);
  check((await style(page, "s1", "opacity")) === "1", "second click shows s1");
  check((await state(page)).step === 2, "two clicks played");
  await page.keyboard.press("ArrowRight");
  await wait(50);
  check((await state(page)).slide === 2, "a third click goes to the next slide");
  // FR-ANIM-03: slide 4 has transition=push.
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await wait(100);
  const pushed = await page.evaluate(() => Array.from(document.querySelectorAll("section.smd-slide")).map((s) => s.style.transform).filter(Boolean));
  check(pushed.some((t) => t.startsWith("translateX(")), `FR-ANIM-03: push moves the slides: ${pushed.join(" | ")}`);
  await wait(600);
  await page.keyboard.press("End");
  await page.keyboard.press("ArrowRight");
  await wait(50);
  check(await page.$eval("#smd-end", (e) => !e.hidden), "end screen after the last slide");
  await page.keyboard.press("Home");
  await wait(50);
  check((await state(page)).slide === 0, "Home goes to the first slide");
  // Buttons work by keyboard too.
  await page.focus("#smd-next");
  await page.keyboard.press("Enter");
  await wait(50);
  check((await state(page)).slide === 1, "next button works from the keyboard");
  check(problems.length === 0, `no console errors or CSP violations: ${problems.join("; ")}`);
  check(await page.evaluate(() => [...document.images].every((i) => i.src.startsWith("data:") && i.complete && i.naturalWidth > 0)), "every image is embedded and decodes");
  await page.close();
}

// Reduced motion: steps and transitions are instant.
{
  const { page } = await open(pitch + "#2", { reduced: true });
  await page.keyboard.press("ArrowRight");
  await wait(20);
  check((await style(page, "b1", "transform")) === "", "prefers-reduced-motion: the step is already at its end");
  await page.close();
}

// FR-DIR-02: rtl deck, Left Arrow goes forward and positions don't mirror.
{
  const { page, problems } = await open(carousel);
  const left = await style(page, "t1", "left");
  await page.keyboard.press("ArrowLeft");
  await wait(50);
  await page.keyboard.press("ArrowLeft");
  await wait(50);
  check((await state(page)).slide === 1, "rtl: Left Arrow goes to the next slide");
  await page.keyboard.press("ArrowRight");
  await wait(50);
  check((await state(page)).slide === 0, "rtl: Right Arrow goes back");
  check((await style(page, "t1", "left")) === left && left === "80px", "rtl: elements keep their x");
  check(problems.length === 0, `rtl deck: no console errors: ${problems.join("; ")}`);
  await page.close();
}

// Presenter view: current slide, next slide, notes and timer.
{
  const { page } = await open(pitch + "#presenter");
  await wait(300);
  const notes = await page.$eval(".smd-pv-notes", (e) => e.textContent);
  check(notes.includes("Open with the problem"), "presenter view shows the notes");
  check(await page.$eval(".smd-pv-next section", (s) => s.dataset.slide === "1"), "presenter view shows the next slide");
  check(/\d\d:\d\d/.test(await page.$eval(".smd-pv-timer span", (e) => e.textContent)), "presenter view shows a timer");
  const onSlide = await page.$eval("#smd-stage", (e) => e.textContent);
  check(!onSlide.includes("Open with the problem"), "FR-NOTES-01: notes never on the slide");
  await page.close();
}

// Accessibility: axe-core, WCAG 2.2 AA, on the presentation controls.
for (const url of [pitch, carousel]) {
  const { page } = await open(url, { bypassCSP: true });
  await page.addScriptTag({ content: axeSource });
  const result = await page.evaluate(() =>
    window.axe.run(
      { include: [[".smd-controls"], ["#smd-progress"]] },
      { runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] } },
    ),
  );
  const v = result.violations.map((x) => `${x.id} (${x.nodes.length})`);
  check(v.length === 0, `axe WCAG 2.2 AA on the controls of ${url.split("/").pop()}: ${v.join(", ") || "no violations"}`);
  await page.close();
}

await browser.close();
writeFileSync(join(out, "README.txt"), "Exports from runtime/test/check.mjs\n");
console.log(failures ? `${failures} check(s) failed` : "all checks passed");
process.exit(failures ? 1 : 0);
