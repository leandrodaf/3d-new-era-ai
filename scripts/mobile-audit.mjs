#!/usr/bin/env node
// What a phone actually gets: loads a page at real handset sizes and reports
// what would go wrong there — the page scrolling sideways, an element wider
// than the screen, text too small to read, a tap target too small to hit —
// with a screenshot of each size for the eye.
//
//   node scripts/mobile-audit.mjs http://127.0.0.1:8801/ out-dir
//
// Exits non-zero when something is broken, so it can gate a release.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [url = "http://127.0.0.1:8801/", out = "target/mobile"] = process.argv.slice(2);
mkdirSync(out, { recursive: true });

// The sizes people actually hold, smallest first. `dpr` matters: the layout is
// in CSS pixels, the screenshot in device pixels.
const PHONES = [
  { name: "iphone-se", width: 375, height: 667, dpr: 2 },
  { name: "iphone-14", width: 390, height: 844, dpr: 3 },
  { name: "android", width: 360, height: 800, dpr: 3 },
  { name: "ipad-mini", width: 768, height: 1024, dpr: 2 },
];

const profile = mkdtempSync(join(tmpdir(), "newera-mobile-"));
const browser = process.env.CHROME
  ?? ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "/Applications/Chromium.app/Contents/MacOS/Chromium"].find((p) => existsSync(p))
  ?? "google-chrome";
const chrome = spawn(browser, [
  "--headless=new", `--user-data-dir=${profile}`, "--use-angle=swiftshader",
  "--remote-debugging-port=9444", "about:blank",
], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let targets;
for (let i = 0; i < 150 && !targets; i++) {
  await sleep(200);
  targets = await fetch("http://127.0.0.1:9444/json").then((r) => r.json()).catch(() => null);
}
const page = targets?.find((t) => t.type === "page");
if (!page) {
  console.error("Chrome did not open a debuggable page in 30 s");
  process.exit(1);
}
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r));
let id = 0;
const pending = new Map();
ws.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  if (pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
});
const send = (method, params = {}) => new Promise((r) => {
  pending.set(++id, r);
  ws.send(JSON.stringify({ id, method, params }));
});
const evaluate = async (expression) =>
  (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }))
    .result?.result?.value;

// Everything the page can tell about itself on this screen, in one go.
const INSPECT = `(() => {
  const view = document.documentElement.clientWidth;
  const doc = document.documentElement;
  const wide = [];
  const small = [];
  const tiny = [];
  const seen = new Set();
  const name = (el) => {
    const id = el.id ? "#" + el.id : "";
    const cls = typeof el.className === "string" && el.className
      ? "." + el.className.trim().split(/\\s+/).slice(0, 2).join(".")
      : "";
    return el.tagName.toLowerCase() + id + cls;
  };
  for (const el of document.querySelectorAll("body *")) {
    const style = getComputedStyle(el);
    if (style.display === "none" || style.visibility === "hidden" || Number(style.opacity) === 0) continue;
    const box = el.getBoundingClientRect();
    if (!box.width && !box.height) continue;
    // Wider than the screen, or hanging off its right edge.
    if (box.width > view + 1 || box.right > view + 1) {
      const key = name(el);
      if (!seen.has(key)) {
        seen.add(key);
        wide.push({ el: key, width: Math.round(box.width), right: Math.round(box.right) });
      }
    }
    // Things a finger has to hit.
    const tappable = el.matches("a, button, summary, [role=tab], input, select, details > summary");
    if (tappable && (box.height < 40 || box.width < 32) && box.height > 0) {
      small.push({ el: name(el), w: Math.round(box.width), h: Math.round(box.height) });
    }
    // Text too small to read on a phone.
    const text = el.children.length === 0 ? (el.textContent || "").trim() : "";
    if (text.length > 8) {
      const size = parseFloat(style.fontSize);
      if (size && size < 12) tiny.push({ el: name(el), size, text: text.slice(0, 32) });
    }
  }
  return {
    view,
    scrollWidth: doc.scrollWidth,
    overflow: doc.scrollWidth - view,
    wide: wide.slice(0, 12),
    small: small.slice(0, 12),
    tiny: tiny.slice(0, 8),
    title: document.title,
  };
})()`;

let bad = 0;
for (const phone of PHONES) {
  await send("Emulation.setDeviceMetricsOverride", {
    width: phone.width, height: phone.height, deviceScaleFactor: phone.dpr, mobile: true,
  });
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Page.navigate", { url });
  await sleep(2500);
  // Let everything that appears on scroll appear.
  await evaluate("window.scrollTo(0, document.body.scrollHeight); true");
  await sleep(1200);
  await evaluate("window.scrollTo(0, 0); true");
  await sleep(600);
  const report = await evaluate(INSPECT);
  const shot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true });
  writeFileSync(join(out, `${phone.name}.png`), Buffer.from(shot.result.data, "base64"));

  const problems = [];
  if (report.overflow > 1) problems.push(`page scrolls sideways by ${report.overflow}px`);
  for (const w of report.wide) problems.push(`${w.el} is ${w.width}px wide (screen ${report.view}px)`);
  for (const s of report.small) problems.push(`${s.el} is only ${s.w}×${s.h}px to tap`);
  for (const t of report.tiny) problems.push(`${t.el} sets ${t.size}px text — "${t.text}"`);
  console.log(`\n${phone.name} (${phone.width}×${phone.height} @${phone.dpr}x) — ${report.title}`);
  if (problems.length === 0) {
    console.log("  ok");
  } else {
    bad += problems.length;
    for (const p of problems) console.log(`  ✗ ${p}`);
  }
}

ws.close();
chrome.kill();
process.exit(bad ? 1 : 0);
