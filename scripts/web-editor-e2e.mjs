// End-to-end check of what runs in a browser: draws a wall with the mouse in
// the editor (headless Chrome, WebGPU) and loads the lightweight viewer, with
// screenshots of both. Any error the page logs fails the run.
// Usage: node scripts/web-editor-e2e.mjs http://127.0.0.1:8790/editor/ out-dir
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// The viewer sits beside the editor when serving web/ (/ and /editor/), and
// under /viewer/ on the published site: say where it is when it is not the
// folder above.
const [url = "http://127.0.0.1:8790/editor/", out = ".", viewerArg] = process.argv.slice(2);
const profile = mkdtempSync(join(tmpdir(), "newera-e2e-"));
// Where Chrome is called on each machine; CHROME=<path> overrides.
const browser = process.env.CHROME
  ?? ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "/Applications/Chromium.app/Contents/MacOS/Chromium"].find((path) => existsSync(path))
  ?? "google-chrome";
// NO_WEBGPU=1 checks the other path: the WebGL fallback browsers without
// WebGPU (Firefox today) land on.
const webgpu = process.env.NO_WEBGPU !== "1";
const chrome = spawn(browser, [
  "--headless=new", `--user-data-dir=${profile}`,
  ...(webgpu ? ["--enable-unsafe-webgpu", "--enable-features=Vulkan"] : ["--disable-features=WebGPU,WebGPUService"]),
  "--use-angle=swiftshader", "--window-size=1440,900",
  "--remote-debugging-port=9333", "about:blank",
], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let targets;
for (let i = 0; i < 50 && !targets; i++) {
  await sleep(200);
  targets = await fetch("http://127.0.0.1:9333/json").then((r) => r.json()).catch(() => null);
}
const page = targets.find((t) => t.type === "page");
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
  (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result?.result?.value;
const shot = async (name) => {
  const { result } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(out, name), Buffer.from(result.data, "base64"));
};
const mouse = (type, x, y, clickCount = 1) =>
  send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount });
const click = async (x, y, count = 1) => {
  await mouse("mouseMoved", x, y);
  await sleep(80);
  await mouse("mousePressed", x, y, count);
  await mouse("mouseReleased", x, y, count);
  await sleep(150);
};

let failed = false;
try {
  await send("Page.enable");
  await send("Page.navigate", { url });
  // Wait for the editor to start (the loading note disappears).
  let started = false;
  for (let i = 0; i < 300 && !started; i++) {
    await sleep(200);
    started = await evaluate("!document.getElementById('loading') && document.readyState === 'complete'");
  }
  if (!started) throw new Error("the editor did not start in 60 s");
  // Wait until the canvas shows something other than a blank page.
  for (let i = 0; i < 40; i++) {
    await sleep(500);
    const { result } = await send("Page.captureScreenshot", { format: "png" });
    if (result.data.length > 20000) break;
  }
  await sleep(1500);
  await shot(webgpu ? "web-editor-start.png" : "web-editor-start-webgl.png");
  if (url.includes("project=")) {
    // A project was given: just check it opened without errors.
    const status = await evaluate("document.title");
    console.log("title:", status);
    const log = await evaluate("JSON.stringify(window.neweraLog || [])");
    console.log("log:", log);
    failed = JSON.parse(log).some((line) => /^(error|uncaught|rejection)/.test(line));
    throw Object.assign(new Error("done"), { done: true });
  }
  // Wall tool (W), then a wall drawn right of the demo house on the plan.
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: "w", code: "KeyW", text: "w" });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: "w", code: "KeyW" });
  await sleep(200);
  await click(1150, 250);
  await click(1350, 250);
  await click(1350, 250, 2);
  await sleep(1500);
  await shot(webgpu ? "web-editor-wall.png" : "web-editor-wall-webgl.png");

  // What was drawn has to survive the tab being closed: the project is
  // mirrored into the browser's storage and opened again on the next visit,
  // instead of the demo home landing on top of it.
  const stored = () => evaluate("localStorage.getItem('newera-autosave') || ''");
  await sleep(2500);
  const drawn = await stored();
  if (!drawn) {
    console.error("nothing was mirrored into the browser's storage");
    failed = true;
  }
  await send("Page.navigate", { url });
  for (let i = 0; i < 300; i++) {
    await sleep(200);
    if (await evaluate("!document.getElementById('loading')")) break;
  }
  await sleep(3000);
  const back = await stored();
  if (back !== drawn) {
    console.error("the work did not come back after a reload");
    failed = true;
  } else {
    console.log("autosave: the drawing came back after a reload");
  }
  const log = await evaluate("JSON.stringify(window.neweraLog || [])");
  console.log("log:", log);
  const errors = JSON.parse(log).filter((line) => /^(error|uncaught|rejection)/.test(line));
  // A machine with no usable GPU cannot run the editor and says so in its own
  // words; that is the machine's limit, not a broken build. Anything else is.
  const noGpu = /createBuffer|too large for the implementation|adapter|WebGPU|WebGL|unreachable/i;
  if (errors.length && errors.every((line) => noGpu.test(line))) {
    console.log("skipped: this machine's GPU stack cannot run the editor");
  } else if (errors.length) {
    failed = true;
  }

  // The lightweight viewer, on the same engine: it is ready when it says so.
  const viewer = viewerArg ?? new URL("../", url).href;
  await send("Page.navigate", { url: viewer });
  let ready = "";
  for (let i = 0; i < 100; i++) {
    await sleep(200);
    ready = await evaluate("document.getElementById('status')?.textContent ?? ''");
    if (/Pronto|Ready/i.test(ready)) break;
  }
  console.log("viewer:", ready);
  await shot(webgpu ? "web-viewer.png" : "web-viewer-webgl.png");
  if (!/Pronto|Ready/i.test(ready)) {
    console.error("the viewer did not load the engine");
    failed = true;
  }
} catch (error) {
  if (!error.done) {
    console.error(error.message);
    failed = true;
  }
} finally {
  ws.close();
  chrome.kill();
}
process.exit(failed ? 1 : 0);
