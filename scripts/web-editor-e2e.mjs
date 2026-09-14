// End-to-end check of the browser editor: draws a wall with the mouse in
// headless Chrome (WebGPU) and saves screenshots before and after.
// Usage: node scripts/web-editor-e2e.mjs http://127.0.0.1:8790/editor/ out-dir
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [url = "http://127.0.0.1:8790/editor/", out = "."] = process.argv.slice(2);
const profile = mkdtempSync(join(tmpdir(), "newera-e2e-"));
const chrome = spawn("google-chrome", [
  "--headless=new", `--user-data-dir=${profile}`, "--enable-unsafe-webgpu",
  "--enable-features=Vulkan", "--use-angle=swiftshader", "--window-size=1440,900",
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
  await sleep(3000);
  await shot("web-editor-start.png");
  // Wall tool (W), then a wall drawn right of the demo house on the plan.
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: "w", code: "KeyW", text: "w" });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: "w", code: "KeyW" });
  await sleep(200);
  await click(1150, 250);
  await click(1350, 250);
  await click(1350, 250, 2);
  await sleep(1500);
  await shot("web-editor-wall.png");
  const log = await evaluate("JSON.stringify(window.neweraLog || [])");
  console.log("log:", log);
  failed = log !== "[]";
} catch (error) {
  console.error(error.message);
  failed = true;
} finally {
  ws.close();
  chrome.kill();
}
process.exit(failed ? 1 : 0);
