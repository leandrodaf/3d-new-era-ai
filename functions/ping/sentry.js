// Crash reports from builds that hold no key: https://3dneweraai.com/ping/sentry
//
// Sentry's own client posts an envelope straight to the ingest host, signed
// with the DSN baked into the binary. A build made from a clone of this
// repository has no DSN, so a crash there was never seen by anybody — and
// those are exactly the builds of the people who compile it themselves.
//
// This is Sentry's tunnel, the same idea its browser SDK uses: the envelope
// arrives here as it left the app, this signs it with the DSN the Pages
// project holds and forwards it unchanged. The binary carries no key, and the
// reports land in the same project as the released builds'.
//
// Environment (Pages project, Production and Preview):
//   NEWERA_SENTRY_DSN   the same DSN the released builds are built with

/** Crash reports are small; an envelope far over this is not one of ours. */
const MAX_BODY = 1024 * 1024;

const NEWLINE = 0x0a;

/**
 * Where an envelope signed with `dsn` has to go, and how to sign it.
 * `null` when the DSN does not parse.
 */
export function ingest(dsn) {
  let url;
  try {
    url = new URL(dsn);
  } catch {
    return null;
  }
  const key = url.username;
  const project = url.pathname.replace(/^\/+/, '');
  if (!key || !/^\d+$/.test(project)) return null;
  return {
    url: `${url.protocol}//${url.host}/api/${project}/envelope/`,
    auth: `Sentry sentry_version=7, sentry_client=newera-tunnel/1.0, sentry_key=${key}`,
  };
}

/**
 * The envelope with the `dsn` of its header removed, so the DSN this endpoint
 * signs with is the only one in play. Sentry reads the project from the URL;
 * the field is optional and the app's placeholder has no business arriving.
 * Anything that is not an envelope comes back as `null`.
 */
export function withoutDsn(bytes) {
  const end = bytes.indexOf(NEWLINE);
  if (end < 0) return null;
  let header;
  try {
    header = JSON.parse(new TextDecoder().decode(bytes.subarray(0, end)));
  } catch {
    return null;
  }
  if (!header || typeof header !== 'object' || Array.isArray(header)) return null;
  if (!('dsn' in header)) return bytes;
  delete header.dsn;
  const rewritten = new TextEncoder().encode(JSON.stringify(header));
  const rest = bytes.subarray(end);
  const out = new Uint8Array(rewritten.length + rest.length);
  out.set(rewritten);
  out.set(rest, rewritten.length);
  return out;
}

export async function onRequestPost({ request, env }) {
  const target = ingest((env.NEWERA_SENTRY_DSN || '').trim());
  // No DSN configured: accept and drop, the way a build without one does.
  if (!target) return new Response(null, { status: 204 });

  const length = Number(request.headers.get('content-length') || 0);
  if (length > MAX_BODY) return new Response('too large', { status: 413 });

  const body = new Uint8Array(await request.arrayBuffer());
  if (body.length > MAX_BODY) return new Response('too large', { status: 413 });
  const envelope = withoutDsn(body);
  if (!envelope) return new Response('not an envelope', { status: 400 });

  const answer = await fetch(target.url, {
    method: 'POST',
    headers: {
      'content-type': 'application/x-sentry-envelope',
      'x-sentry-auth': target.auth,
    },
    body: envelope,
  });
  return new Response(null, { status: answer.ok ? 204 : 502 });
}
