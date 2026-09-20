import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createServer} from 'node:http';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';
import {launchChrome, connectPage} from './chrome-session.mjs';

const fixture = `
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const [mode, meta] = process.argv.slice(2);
const profile = process.argv.find(a => a.startsWith('--user-data-dir=')).split('=').slice(1).join('=');
fs.writeFileSync(meta, JSON.stringify({pid:process.pid, profile}));
if (mode === 'exit') { console.error('fixture: GPU startup refused'); process.exit(23); }
else if (mode === 'stall') { console.error('fixture: endpoint never published'); setInterval(() => {}, 1000); }
else {
  if (mode === 'descendant') {
    const code = "const fs=require('node:fs'); const path=require('node:path'); const [profile,pulse]=process.argv.slice(1); process.on('SIGTERM',()=>{}); setInterval(()=>{fs.mkdirSync(path.join(profile,'Default'),{recursive:true});fs.writeFileSync(path.join(profile,'Default','active'),String(Date.now()));fs.writeFileSync(pulse,String(Date.now()));},10);";
    const child = require('node:child_process').spawn(process.execPath,['-e',code,profile,meta+'.pulse'],{stdio:'ignore'});
    fs.writeFileSync(meta,JSON.stringify({pid:process.pid,profile,descendant:child.pid}));
  }
  let requests = 0;
  const server = http.createServer((req,res) => {
    res.setHeader('content-type','application/json');
    res.end(JSON.stringify(++requests < 3 ? [] : [{type:'page',webSocketDebuggerUrl:'ws://127.0.0.1:1/fake'}]));
  });

  server.listen(0,'127.0.0.1',() => fs.writeFileSync(path.join(profile,'DevToolsActivePort'), String(server.address().port) + '\\n/devtools/browser/test'));
}
`;

async function withFixture(mode, action) {
  const dir = mkdtempSync(join(tmpdir(), 'newera-chrome-test-'));
  const script = join(dir, 'fake.cjs');
  const meta = join(dir, 'meta.json');
  writeFileSync(script, fixture);
  try { await action({browser: process.execPath, prefixArgs: [script, mode, meta], timeoutMs: 3000}, meta); }
  finally { rmSync(dir, {recursive:true,force:true}); }
}
function assertCleaned(meta) {
  const {pid,profile} = JSON.parse(readFileSync(meta,'utf8'));
  assert.equal(existsSync(profile),false,'temporary Chrome profile removed');
  assert.throws(() => process.kill(pid,0), /ESRCH/, 'browser process has exited');
}

test('waits for a page after empty target lists and closes its own process/profile', async () => {
  await withFixture('ready', async (options,meta) => {
    const browser = await launchChrome(options);
    try { assert.equal(browser.page.type,'page'); }
    finally { await browser.close(); }
    await browser.close(); // cleanup is idempotent
    assertCleaned(meta);
  });
});

test('reports startup exit code and stderr, without leaving a process/profile', async () => {
  await withFixture('exit', async (options,meta) => {
    await assert.rejects(launchChrome(options), /code 23[\s\S]*GPU startup refused/);
    assertCleaned(meta);
  });
});

test('times out an unresponsive startup and terminates the browser', async () => {
  await withFixture('stall', async (options,meta) => {
    await assert.rejects(launchChrome({...options,timeoutMs:1000}), /debuggable page[\s\S]*endpoint never published/);
    assertCleaned(meta);
  });
});

test('reports a missing browser executable promptly', async () => {
  await assert.rejects(launchChrome({browser:join(tmpdir(),'newera-missing-chrome-executable')}), /ENOENT/);
});

test('real browsers use separate endpoints and reject protocol errors/disconnection',
  {skip: process.env.RUN_CHROME_TESTS !== '1'}, async () => {
    let foreignRequests = 0;
    const foreign = createServer((req,res) => { foreignRequests++; res.end('[]'); });
    await new Promise((resolve,reject) => {
      foreign.once('error', error => error.code === 'EADDRINUSE' ? resolve() : reject(error));
      foreign.listen(9444,'127.0.0.1',resolve);
    });
    const browsers = [];
    const clients = [];
    try {
      for (let i=0; i<2; i++) {
        const browser = await launchChrome();
        browsers.push(browser);
        clients.push(await connectPage(browser.page));
      }
      assert.notEqual(new URL(browsers[0].page.webSocketDebuggerUrl).port,
        new URL(browsers[1].page.webSocketDebuggerUrl).port);
      for (const client of clients) {
        const reply = await client.send('Runtime.evaluate',{expression:'6 * 7',returnByValue:true});
        assert.equal(reply.result.result.value,42);
        await assert.rejects(client.send('MissingDomain.noSuchMethod'), /MissingDomain.noSuchMethod/);
      }
      const rejected = assert.rejects(clients[0].send('Runtime.evaluate', {
        expression:'new Promise(() => {})',awaitPromise:true,
      }), /connection closed/);
      await browsers[0].close();
      await rejected;
      await assert.rejects(clients[0].send('Runtime.evaluate',{expression:'1'}), /connection closed/);
      assert.equal(foreignRequests,0,'never attach to the fixed port');
    } finally {
      for (const client of clients) client.close();
      for (const browser of browsers) {
        await browser.close();
        assert.equal(existsSync(browser.profile),false);
        assert.throws(() => process.kill(browser.pid,0), /ESRCH/);
      }
      if (foreign.listening) await new Promise(resolve => foreign.close(resolve));
    }
  });

test('mobile audit fails when navigation opens a browser error page',
  {skip: process.env.RUN_CHROME_TESTS !== '1'}, async () => {
    const out = mkdtempSync(join(tmpdir(),'newera-mobile-failure-'));
    try {
      await assert.rejects(promisify(execFile)(process.execPath, [
        fileURLToPath(new URL('./mobile-audit.mjs',import.meta.url)),
        'http://127.0.0.1:1/',out,
      ], {timeout:20000}), error => {
        assert.equal(error.code,1);
        assert.match(error.stderr,/Navigation failed: net::ERR_UNSAFE_PORT/);
        return true;
      });
    } finally { rmSync(out,{recursive:true,force:true}); }
  });

test('cleanup stops a descendant that outlives its parent and writes the profile',
  {skip: process.platform === 'win32'}, async () => {
    await withFixture('descendant', async (options,meta) => {
      const browser = await launchChrome(options);
      const {descendant,profile} = JSON.parse(readFileSync(meta,'utf8'));
      try {
        assert.ok(existsSync(meta+'.pulse'),'descendant is actively writing');
        await browser.close();
        const pulse = readFileSync(meta+'.pulse','utf8');
        await new Promise(resolve => setTimeout(resolve,200));
        assert.equal(readFileSync(meta+'.pulse','utf8'),pulse,'descendant stopped writing');
        assertCleaned(meta);
      } finally {
        try { process.kill(descendant,'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
        await browser.close();
        rmSync(profile,{recursive:true,force:true,maxRetries:10,retryDelay:100});
      }
    });
  });
