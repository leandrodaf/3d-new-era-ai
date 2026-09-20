import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const html = readFileSync(new URL('../web/editor/index.html', import.meta.url), 'utf8');
const source = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)]
  .map(m => m[1]).find(s => s.includes('const WORDS'));
function page() {
  const nodes = new Map();
  const node = id => {
    if (!nodes.has(id)) nodes.set(id, {textContent:'', style:{}, remove(){}, addEventListener(){}});
    return nodes.get(id);
  };
  const events = {};
  const context = {
    navigator:{languages:['pt-BR']}, localStorage:{getItem:()=> 'seen'},
    document:{documentElement:{}, getElementById:node,
      body:{getAttribute:key => key === 'data-render-build' ? 'test-build' : 'BrowserWebGpu'}},
    addEventListener:(name, fn) => {events[name] = fn;}, setTimeout(){},
  };
  context.window = context;
  vm.runInNewContext(source, context);
  return {context, node, events};
}
{
  const {context:c} = page();
  c.neweraFail('WASM startup failed');
  assert.equal(c.neweraDiagnostic.phase, 'startup');
}
{
  const {context:c, node, events} = page();
  c.neweraReady = true;
  c.neweraLog = ['error: panicked at no filesystem on this platform'];
  const render = c.neweraOperation('begin', null, 'render_plan', '42');
  const read = c.neweraOperation('begin', null, 'get_home', '42');
  c.neweraOperation('end', read);
  events.error({message:'Uncaught RuntimeError: unreachable https://relay/secret?token=secret authorization=secret Bearer secret {"tab_key":"secret"}'});
  const d = c.neweraDiagnostic;
  assert.equal(d.phase, 'runtime');
  assert.match(d.panic, /no filesystem/);
  assert.equal(d.build, 'test-build');
  assert.equal(d.backend, 'BrowserWebGpu');
  assert.equal(d.operations.length, 1);
  assert.equal(d.operations[0].tool, 'render_plan');
  assert.equal(d.operations[0].revision, '42');
  assert(!JSON.stringify(d).includes('secret'));
  assert.match(node('failed-text').textContent, /durante o uso/);
  c.neweraFail('secondary failure');
  assert.equal(c.neweraDiagnostic, d, 'first cause survives subsequent errors');
  c.neweraOperation('end', render);
  assert.equal(d.operations.length, 1, 'failure snapshot survives cleanup');
}
console.log('Editor diagnostics: startup, runtime, concurrent operations and redaction passed');

{
  class Socket extends EventTarget {
    readyState=1;
    sent=[];
    send(data) {this.sent.push(JSON.parse(data));}
    call(id,name) {this.dispatchEvent(new MessageEvent('message',{data:JSON.stringify({type:'call',id,name,args:{secret:'never retained'}})}));}
  }
  const {context:c} = page();
  c.neweraReady=true;
  c.neweraRecovery={state:'saved',saved_revision:42,saved_at:123,secret:'never retained'};
  const socket=new Socket();
  c.neweraAttachSocket(socket);
  for (const data of ['null','not JSON','{"type":"client"}']) socket.dispatchEvent(new MessageEvent('message',{data}));
  let rustCalls=0;
  socket.addEventListener('message',()=>rustCalls++);
  socket.call(1,'get_home');
  socket.send(JSON.stringify({type:'result',id:1,ok:true,result:{content:[]}}));
  socket.call(2,'render_plan');
  socket.call(3,'measure');
  c.neweraFail('RuntimeError: unreachable token=secret');
  assert.equal(socket.sent.length,3);
  assert.deepEqual(socket.sent.slice(1).map(x=>x.id),[2,3]);
  const failure=JSON.parse(socket.sent[1].result.content[0].text);
  assert.equal(failure.kind,'editor_runtime_failure');
  assert.equal(failure.diagnostic.recovery.saved_revision,42);
  assert.deepEqual(failure.diagnostic.operations.map(o=>o.tool),['render_plan','measure']);
  assert(!JSON.stringify(failure).includes('secret'));
  socket.send(JSON.stringify({type:'result',id:2,ok:true,result:{}}));
  assert.equal(socket.sent.length,3,'late WASM replies must not answer twice');
  socket.call(4,'get_home');
  assert.equal(socket.sent.length,4);
  assert.equal(rustCalls,3,'future calls must not enter the failed WASM handler');
  socket.readyState=3;
  socket.dispatchEvent(new Event('close'));
}
console.log('Emergency MCP responder: pending calls, future calls, late replies and redaction passed');
