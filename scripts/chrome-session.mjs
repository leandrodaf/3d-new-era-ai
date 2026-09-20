import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const defaultBrowser = () => process.env.CHROME
  ?? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/Applications/Chromium.app/Contents/MacOS/Chromium'].find(existsSync)
  ?? 'google-chrome';

// The port file belongs to this disposable profile: never attach to another
// test's browser, or race its shutdown by reusing a fixed debugging port.
export async function launchChrome({browser = defaultBrowser(), prefixArgs = [], timeoutMs = 30000} = {}) {
  const profile = mkdtempSync(join(tmpdir(), 'newera-mobile-'));
  const child = spawn(browser, [...prefixArgs, '--headless=new', `--user-data-dir=${profile}`,
    '--use-angle=swiftshader', '--remote-debugging-port=0', 'about:blank'],
    {stdio: ['ignore', 'ignore', 'pipe'], detached: process.platform !== 'win32'});
  let stderr = '';
  let failure;
  let exited = false;
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-16384); });
  const stopped = new Promise(resolve => {
    child.once('error', error => { failure = error.message; exited = true; resolve(); });
    child.once('exit', (code, signal) => {
      failure = `Chrome exited (code ${code}, signal ${signal ?? 'none'})`;
      exited = true;
      resolve();
    });
  });
  const diagnostics = () => [failure, stderr.trim()].filter(Boolean).join('\n');
  let closed = false;
  const signalTree = signal => {
    if (!child.pid) return;
    try {
      if (process.platform === 'win32') child.kill(signal);
      else process.kill(-child.pid, signal);
    } catch (error) { if (error.code !== 'ESRCH') throw error; }
  };
  const close = async () => {
    if (closed) return;
    closed = true;
    // Chromium subprocesses may still write the profile after its parent
    // exits. This process group belongs exclusively to this launch.
    signalTree('SIGTERM');
    if (!exited) {
      await Promise.race([stopped, sleep(3000, undefined, {ref: false})]);
    }
    signalTree('SIGKILL');
    await stopped;
    rmSync(profile, {recursive: true, force: true, maxRetries: 10, retryDelay: 100});
  };
  try {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (exited) throw new Error(failure);
      let port;
      try { port = Number(readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]); }
      catch { /* Chrome has not published its endpoint yet. */ }
      if (Number.isInteger(port) && port > 0 && port <= 65535) {
        const targets = await fetch(`http://127.0.0.1:${port}/json`, {
          signal: AbortSignal.timeout(Math.max(1, Math.min(1000, deadline - Date.now()))),
        }).then(r => r.ok ? r.json() : null).catch(() => null);
        const page = Array.isArray(targets) && targets.find(t => t.type === 'page' && t.webSocketDebuggerUrl);
        if (page) return {page, profile, pid: child.pid, close, diagnostics};
      }
      await sleep(Math.min(100, Math.max(1, deadline - Date.now())));
    }
    throw new Error(`Chrome did not open a debuggable page in ${timeoutMs} ms`);
  } catch (error) {
    const detail = diagnostics();
    await close();
    throw new Error(`${error.message}${detail ? `\n${detail}` : ''}`);
  }
}

export async function connectPage(page) {
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => { ws.close(); reject(new Error('Chrome WebSocket connection timed out')); }, 10000);
    ws.addEventListener('open', () => { clearTimeout(timer); resolve(); }, {once: true});
    ws.addEventListener('error', () => { clearTimeout(timer); reject(new Error('Chrome WebSocket connection failed')); }, {once: true});
  });
  let nextId = 0;
  const pending = new Map();
  const rejectAll = () => {
    for (const request of pending.values()) request.reject(new Error('Chrome connection closed during a command'));
    pending.clear();
  };
  ws.addEventListener('close', rejectAll);
  ws.addEventListener('error', rejectAll);
  ws.addEventListener('message', event => {
    const message = JSON.parse(event.data);
    const request = pending.get(message.id);
    if (!request) return;
    if (message.error) request.reject(new Error(`${request.method}: ${message.error.message}`));
    else request.resolve(message);
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    if (ws.readyState !== WebSocket.OPEN) {
      reject(new Error('Chrome connection closed before a command'));
      return;
    }
    const id = ++nextId;
    const finish = (callback, value) => { clearTimeout(timer); pending.delete(id); callback(value); };
    const timer = setTimeout(() => finish(reject, new Error(`${method} timed out after 20 s`)), 20000);
    pending.set(id, {method, resolve: value => finish(resolve, value), reject: error => finish(reject, error)});
    try { ws.send(JSON.stringify({id, method, params})); }
    catch (error) { finish(reject, error); }
  });
  return {send, close: () => { rejectAll(); ws.close(); }};
}
