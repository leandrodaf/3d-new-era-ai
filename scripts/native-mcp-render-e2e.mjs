// Run against a native server with the reviewed demo loaded. Read-only.
// node scripts/native-mcp-render-e2e.mjs http://127.0.0.1:7913/mcp
import assert from 'node:assert/strict';

const url = process.argv[2];
if (!url) throw new Error('Pass the native MCP endpoint');
let session;
let nextId = 1;
let pings = 0;
const post = body => fetch(url, {
  method: 'POST',
  headers: {'content-type':'application/json', accept:'application/json, text/event-stream',
    ...(session ? {'mcp-session-id':session} : {})},
  body: JSON.stringify({jsonrpc:'2.0', ...body}),
  signal: AbortSignal.timeout(600000),
});
async function rpc(method, params, onProgress) {
  const id = nextId++;
  const response = await post({id, method, params});
  assert(response.ok, `HTTP ${response.status}`);
  session ??= response.headers.get('mcp-session-id');
  if (response.headers.get('content-type')?.includes('application/json')) return response.json();
  const decoder = new TextDecoder();
  let buffer = '';
  let result;
  for await (const bytes of response.body) {
    buffer += decoder.decode(bytes, {stream:true});
    let end;
    while ((end = buffer.indexOf('\n\n')) >= 0) {
      const event = buffer.slice(0, end);
      buffer = buffer.slice(end + 2);
      const data = event.split('\n').find(l => l.startsWith('data:'))?.slice(5).trim();
      if (!data) continue; // SSE priming/keepalive event
      const value = JSON.parse(data);
      if (value.method === 'ping') {
        pings++;
        const reply = await post({id:value.id, result:{}});
        assert(reply.ok);
        await reply.text();
      } else if (value.method === 'notifications/progress') {
        await onProgress?.(value.params, id);
      } else if (value.id === id) result = value;
    }
  }
  return result;
}
await rpc('initialize', {protocolVersion:'2025-03-26', capabilities:{}, clientInfo:{name:'native-render-e2e',version:'1'}});
await post({method:'notifications/initialized'});
try {
  const photo = await rpc('tools/call', {name:'render_photo',arguments:{cam:0,hour:21,w:480,h:360,quality:'draft'}});
  assert(photo?.result?.content?.some(c => c.type === 'image'), JSON.stringify(photo));
  assert(pings > 0, 'fixture must exercise the server heartbeat during a real render');
  console.log('Long native photo returned an image and answered server pings without client keepalive.');

  let cancelled = false;
  let busyChecked = false;
  await rpc('tools/call', {name:'render_photo',arguments:{cam:0,w:640,h:480,quality:'best'},_meta:{progressToken:'cancel-photo'}}, async (progress, id) => {
    if (cancelled || !progress.message) return;
    const read = await rpc('tools/call', {name:'get_home',arguments:{}});
    assert(read?.result && !read.result.isError, 'reads must remain responsive');
    const busy = await rpc('tools/call', {name:'render_plan',arguments:{w:64,h:64}});
    assert(JSON.stringify(busy).includes('em andamento'), 'second render must be refused');
    busyChecked = true;
    cancelled = true;
    const response = await post({method:'notifications/cancelled',params:{requestId:id,reason:'E2E cancellation'}});
    assert(response.ok);
  });
  assert(cancelled && busyChecked);
  const deadline = Date.now() + 15000;
  let retry;
  do {
    retry = await rpc('tools/call', {name:'render_plan',arguments:{w:64,h:64}});
    if (retry?.result?.content?.some(c => c.type === 'image')) break;
    assert(JSON.stringify(retry).includes('em andamento'), JSON.stringify(retry));
    await new Promise(resolve => setTimeout(resolve, 100));
  } while (Date.now() < deadline);
  assert(retry?.result?.content?.some(c => c.type === 'image'), 'cancelled worker did not release render permit');
  console.log('Real photo cancelled; reads stayed responsive, concurrent render was refused, and next render succeeded.');
} finally {
  await fetch(url, {method:'DELETE', headers:{'mcp-session-id':session}});
}
