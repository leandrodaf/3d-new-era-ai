#!/usr/bin/env node
// What a phone actually gets: loads a page at real handset sizes and reports
// what would go wrong there — the page scrolling sideways, an element wider
// than the screen, text too small to read, a tap target too small to hit —
// with a screenshot of each size for the eye.
//
//   node scripts/mobile-audit.mjs http://127.0.0.1:8801/ out-dir
//
// Exits non-zero when something is broken, so it can gate a release.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { launchChrome, connectPage } from "./chrome-session.mjs";

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

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let browser;
let client;
try {
  browser = await launchChrome();
  client = await connectPage(browser.page);
  const {send} = client;
  const evaluate = async expression => {
    const reply = await send("Runtime.evaluate", {expression, awaitPromise: true, returnByValue: true});
    if (reply.result?.exceptionDetails) throw new Error(`Page evaluation failed: ${JSON.stringify(reply.result.exceptionDetails)}`);
    return reply.result?.result?.value;
  };

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
      // A control the page hides off-screen (the file picker's own input) is not
      // something anybody taps.
      const hidden = box.width < 8 || box.height < 8 || box.bottom < 0 || box.top > innerHeight * 8;
      if (tappable && !hidden && (box.height < 40 || box.width < 32)) {
        small.push({
          el: name(el),
          w: Math.round(box.width),
          h: Math.round(box.height),
          text: (el.textContent || "").trim().slice(0, 24),
          within: el.parentElement ? name(el.parentElement) : "",
        });
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
    const navigation = await send("Page.navigate", { url });
    if (navigation.result?.errorText) throw new Error(`Navigation failed: ${navigation.result.errorText}`);
    await sleep(2500);
    // Let everything that appears on scroll appear.
    await evaluate("window.scrollTo(0, document.body.scrollHeight); true");
    await sleep(1200);
    await evaluate("window.scrollTo(0, 0); true");
    await sleep(600);
    const report = await evaluate(INSPECT);
    if (!report) {
      console.log(`\n${phone.name} — the page did not answer`);
      bad += 1;
      continue;
    }
    const problems = [];
    if (report.overflow > 1) problems.push(`page scrolls sideways by ${report.overflow}px`);
    for (const w of report.wide) problems.push(`${w.el} is ${w.width}px wide (screen ${report.view}px)`);
    for (const s of report.small) {
      problems.push(`${s.el} is only ${s.w}×${s.h}px to tap — "${s.text}" in ${s.within}`);
    }
    for (const t of report.tiny) problems.push(`${t.el} sets ${t.size}px text — "${t.text}"`);
    console.log(`\n${phone.name} (${phone.width}×${phone.height} @${phone.dpr}x) — ${report.title}`);
    if (problems.length === 0) {
      console.log("  ok");
    } else {
      bad += problems.length;
      for (const p of problems) console.log(`  ✗ ${p}`);
    }
    const shot = await send("Page.captureScreenshot", { format: "png" });
    if (!shot.result?.data) throw new Error(`Missing screenshot for ${phone.name}`);
    writeFileSync(join(out, `${phone.name}.png`), Buffer.from(shot.result.data, "base64"));
  }

  process.exitCode = bad ? 1 : 0;
} catch (error) {
  console.error(error.stack ?? error.message);
  const detail = browser?.diagnostics();
  if (detail) console.error(detail);
  process.exitCode = 1;
} finally {
  client?.close();
  await browser?.close();
}
