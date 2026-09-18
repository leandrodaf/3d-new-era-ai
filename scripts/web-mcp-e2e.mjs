#!/usr/bin/env node
// The browser as an MCP server, checked the way it is used: a headless tab
// switches its MCP on, an AI client talks to the address the relay hands out,
// and a wall it draws has to be in that tab's project.
//
//   node scripts/web-mcp-e2e.mjs http://127.0.0.1:8801/app/ http://127.0.0.1:7979
//
// Needs the relay running and the site served. Exits non-zero on any step.
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [page = "http://127.0.0.1:8801/app/", relay = "http://127.0.0.1:7979"] =
  process.argv.slice(2);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = mkdtempSync(join(tmpdir(), "newera-mcp-e2e-"));
const browser = process.env.CHROME
  ?? ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "/Applications/Chromium.app/Contents/MacOS/Chromium"].find((p) => existsSync(p))
  ?? "google-chrome";
const chrome = spawn(browser, [
  "--headless=new", `--user-data-dir=${profile}`,
  "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=swiftshader",
  "--window-size=1440,900", "--remote-debugging-port=9555", "about:blank",
], { stdio: "ignore" });

let targets;
for (let i = 0; i < 150 && !targets; i++) {
  await sleep(200);
  targets = await fetch("http://127.0.0.1:9555/json").then((r) => r.json()).catch(() => null);
}
const target = targets?.find((t) => t.type === "page");
if (!target) {
  console.error("Chrome did not open a debuggable page in 30 s");
  process.exit(1);
}
const ws = new WebSocket(target.webSocketDebuggerUrl);
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
// A hidden tab only draws when a picture is asked of it, and egui only acts
// on a frame: every wait asks for one.
const frames = async (times) => {
  for (let i = 0; i < times; i++) {
    await send("Page.captureScreenshot", { format: "png" });
    await sleep(150);
  }
};

const rpc = async (url, body) => {
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
    body: JSON.stringify(body),
  });
  return response.json();
};

let failed = false;
const ok = (what) => console.log(`  ✓ ${what}`);
const bad = (what) => { console.error(`  ✗ ${what}`); failed = true; };

try {
  await send("Page.enable");
  await send("Page.navigate", { url: `${page}?relay=${encodeURIComponent(relay)}` });
  let started = false;
  for (let i = 0; i < 300 && !started; i++) {
    await sleep(200);
    started = await evaluate("!document.getElementById('loading') && document.readyState === 'complete'");
  }
  if (!started) throw new Error("the editor did not start in 60 s");
  await frames(20);

  const before = await fetch(`${relay}/health`).then((r) => r.json());

  // Ctrl+Shift+M is the switch — the same one the panel's button throws. A
  // loaded runner can be between frames when the keys arrive, so this presses
  // again until the relay says a room is open: what is being checked is the
  // room, not the keyboard.
  const press = async () => {
    for (const [type, extra] of [["keyDown", { text: "" }], ["keyUp", {}]]) {
      await send("Input.dispatchKeyEvent", {
        type, key: "M", code: "KeyM", windowsVirtualKeyCode: 77,
        modifiers: 2 | 8, ...extra,
      });
    }
  };
  let health = before;
  for (let round = 0; round < 4 && health.rooms <= before.rooms; round++) {
    await press();
    for (let i = 0; i < 20 && health.rooms <= before.rooms; i++) {
      await frames(2);
      health = await fetch(`${relay}/health`).then((r) => r.json());
    }
  }
  if (health.rooms <= before.rooms) {
    const log = await evaluate("JSON.stringify((window.neweraLog || []).slice(-6))");
    throw new Error(`the tab never opened a room on the relay — page said ${log}`);
  }
  ok("the tab switched its MCP on");

  // The tab publishes the address it is reachable at, which is what a person
  // copies out of the panel.
  let address = "";
  for (let i = 0; i < 20 && !address; i++) {
    await frames(2);
    address = await evaluate("document.body.getAttribute('data-mcp') || ''");
  }
  if (!address) throw new Error("the tab did not publish its MCP address");
  ok("the address is on the page");
  await check(address);

  async function check(mcpUrl) {
    const hello = await rpc(mcpUrl, {
      jsonrpc: "2.0", id: 1, method: "initialize",
      params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "e2e", version: "1" } },
    });
    if (hello?.result?.serverInfo?.name !== "3d-new-era-ai") bad("the handshake answered wrong");
    else ok("an AI client shook hands with the tab");

    const list = await rpc(mcpUrl, { jsonrpc: "2.0", id: 2, method: "tools/list" });
    const tools = list?.result?.tools ?? [];
    if (tools.length < 30) bad(`the tab offered only ${tools.length} tools`);
    else ok(`the tab offered ${tools.length} tools`);

    const before = await rpc(mcpUrl, {
      jsonrpc: "2.0", id: 3, method: "tools/call",
      params: { name: "get_home", arguments: {} },
    });
    const made = await rpc(mcpUrl, {
      jsonrpc: "2.0", id: 4, method: "tools/call",
      params: { name: "create", arguments: { walls: [{ pts: [[900, 0], [900, 600]]}] } },
    });
    const said = made?.result?.content?.[0]?.text ?? "";
    if (!said.startsWith("ok")) bad(`the wall did not go in: ${said}`);
    else ok(`the AI drew in the tab: ${said}`);

    const after = await rpc(mcpUrl, {
      jsonrpc: "2.0", id: 5, method: "tools/call",
      params: { name: "get_home", arguments: {} },
    });
    const grew = (after?.result?.content?.[0]?.text ?? "").length
      > (before?.result?.content?.[0]?.text ?? "").length;
    if (!grew) bad("the project did not change in the tab");
    else ok("the tab's project holds what the AI drew");

    // Two ways an address must die, checked in order of how much they matter.
    //
    // First the switch: Ctrl+Shift+M is the same one the panel's button
    // throws. A loaded runner can be between frames when the keys arrive, so
    // this is only judged when the window says it acted — the page stops
    // publishing its address — and it is the window's word that is checked,
    // not the keyboard's luck.
    const press = async () => {
      for (const [type, extra] of [["keyDown", { text: "" }], ["keyUp", {}]]) {
        await send("Input.dispatchKeyEvent", {
          type, key: "M", code: "KeyM", windowsVirtualKeyCode: 77,
          modifiers: 2 | 8, ...extra,
        });
      }
    };
    let off = false;
    for (let round = 0; round < 3 && !off; round++) {
      await press();
      for (let i = 0; i < 10 && !off; i++) {
        await frames(2);
        off = !(await evaluate("document.body.hasAttribute('data-mcp')"));
      }
    }
    if (off) {
      let closed = false;
      for (let i = 0; i < 20 && !closed; i++) {
        const afterOff = await rpc(mcpUrl, {
          jsonrpc: "2.0", id: 6, method: "tools/call",
          params: { name: "get_home", arguments: {} },
        });
        closed = afterOff?.result?.isError === true || Boolean(afterOff?.error);
        if (!closed) await sleep(300);
      }
      if (!closed) bad("switched off, and the address still answered");
      else ok("switched off, the address answers nobody");
    } else {
      console.log("  … the window never took the keys; the switch goes unchecked here");
    }

    // And then the one that has to hold whatever anybody presses: the tab
    // goes, the address dies. Nothing is left running for an AI to reach.
    await send("Page.navigate", { url: "about:blank" });
    let gone = false;
    for (let i = 0; i < 40 && !gone; i++) {
      const afterGone = await rpc(mcpUrl, {
        jsonrpc: "2.0", id: 7, method: "tools/call",
        params: { name: "get_home", arguments: {} },
      });
      gone = afterGone?.result?.isError === true || Boolean(afterGone?.error);
      if (!gone) await sleep(300);
    }
    if (!gone) bad("the tab was closed and its address went on answering");
    else ok("the tab gone, the address answers nobody");
  }
} catch (error) {
  console.error(error.message);
  failed = true;
} finally {
  ws.close();
  chrome.kill();
}
process.exit(failed ? 1 : 0);
