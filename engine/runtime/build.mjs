// Builds the runtime into crates/core/src/runtime/: minified JS and CSS, plus the
// base64 SHA-256 of the JS that the export's Content Security Policy pins.
import { build, transform } from "esbuild";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

const out = new URL("../crates/core/src/runtime/", import.meta.url);
const js = await build({
  entryPoints: [new URL("src/main.ts", import.meta.url).pathname],
  bundle: true,
  minify: true,
  format: "iife",
  target: "es2020",
  write: false,
  legalComments: "none",
});
const code = js.outputFiles[0].text.trimEnd();
const css = await transform(readFileSync(new URL("src/runtime.css", import.meta.url), "utf8"), {
  loader: "css",
  minify: true,
});
writeFileSync(new URL("runtime.js", out), code);
writeFileSync(new URL("runtime.css", out), css.code.trimEnd());
writeFileSync(new URL("runtime.sha256", out), createHash("sha256").update(code).digest("base64"));
const reload = await build({
  entryPoints: [new URL("src/reload.ts", import.meta.url).pathname],
  bundle: true,
  minify: true,
  format: "iife",
  target: "es2020",
  write: false,
});
const reloadCode = reload.outputFiles[0].text.trimEnd();
writeFileSync(new URL("reload.js", out), reloadCode);
writeFileSync(new URL("reload.sha256", out), createHash("sha256").update(reloadCode).digest("base64"));
console.log(`runtime.js ${code.length} bytes, runtime.css ${css.code.length} bytes`);
