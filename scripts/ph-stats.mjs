#!/usr/bin/env node
// How the launch is doing, from the terminal.
//
// Product Hunt hides the vote count for the first four hours and the page is
// heavy to keep reloading; this asks the API instead, and can sit in a corner
// refreshing itself.
//
//   scripts/ph-stats.mjs --slug 3d-new-era-ai
//   scripts/ph-stats.mjs --slug 3d-new-era-ai --watch        # every 2 min
//   scripts/ph-stats.mjs --slug 3d-new-era-ai --json         # for a script
//
// The token is a Product Hunt *developer token*, made at
// https://www.producthunt.com/v2/oauth/applications → your app → Create Token.
// It goes in the environment, never on the command line and never in the repo:
//
//   export PH_TOKEN=...            (or put it in a password manager and use
//                                   `op run -- scripts/ph-stats.mjs ...`)
//
// It reads; it never votes, comments or publishes. Nothing here can launch the
// post — that stays a decision made by a person, in the browser.

const API = "https://api.producthunt.com/v2/api/graphql";

const args = process.argv.slice(2);
const flag = (name, fallback = null) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] && !args[i + 1].startsWith("--") ? args[i + 1] : fallback;
};
const has = (name) => args.includes(`--${name}`);

const slug = flag("slug", "3d-new-era-ai");
const every = Number(flag("every", "120")) * 1000;
const token = process.env.PH_TOKEN || process.env.PRODUCT_HUNT_TOKEN;

if (has("help") || has("h")) {
  console.log(`usage: ph-stats.mjs [--slug <slug>] [--watch] [--every <seconds>] [--json]`);
  process.exit(0);
}
if (!token) {
  console.error("PH_TOKEN is not set — see the header of this file for where the token comes from.");
  process.exit(2);
}

async function ask(query, variables) {
  const res = await fetch(API, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify({ query, variables }),
  });
  if (res.status === 401 || res.status === 403) throw new Error("the token was refused (401/403) — make a fresh developer token");
  if (res.status === 429) throw new Error("rate limited (429) — the API allows a few hundred calls an hour; try --every 300");
  const body = await res.json().catch(() => null);
  if (!res.ok || !body) throw new Error(`HTTP ${res.status}`);
  if (body.errors?.length) throw new Error(body.errors.map((e) => e.message).join("; "));
  return body.data;
}

const POST = `query ($slug: String!) {
  post(slug: $slug) {
    id name tagline url votesCount commentsCount reviewsCount featuredAt createdAt
    topics(first: 5) { edges { node { name } } }
    comments(first: 8, order: NEWEST) {
      edges { node { id createdAt body user { name username } } }
    }
  }
}`;

// v2 has no "rank" field, so the standing is counted the honest way: today's
// posts, ordered by votes, and where ours sits in that list.
const TODAY = `query ($after: String, $postedAfter: DateTime!) {
  posts(order: VOTES, first: 20, after: $after, postedAfter: $postedAfter) {
    pageInfo { hasNextPage endCursor }
    edges { node { id votesCount } }
  }
}`;

async function standing(id) {
  const midnightPT = (() => {
    // Product Hunt's day starts at 00:00 Pacific. Work it out from the offset
    // the runtime reports for Los Angeles, so it holds in summer and winter.
    const now = new Date();
    const there = new Date(now.toLocaleString("en-US", { timeZone: "America/Los_Angeles" }));
    const drift = now.getTime() - there.getTime();
    there.setHours(0, 0, 0, 0);
    return new Date(there.getTime() + drift);
  })();
  let after = null, place = 0, seen = 0;
  for (let page = 0; page < 10; page++) {
    const { posts } = await ask(TODAY, { after, postedAfter: midnightPT.toISOString() });
    for (const { node } of posts.edges) {
      seen++;
      if (node.id === id) place = seen;
    }
    if (place || !posts.pageInfo.hasNextPage) return { place, seen };
    after = posts.pageInfo.endCursor;
  }
  return { place, seen };
}

const ago = (iso) => {
  const mins = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (mins < 60) return `${mins} min`;
  const hours = mins / 60;
  return hours < 48 ? `${hours.toFixed(1)} h` : `${(hours / 24).toFixed(1)} d`;
};

let before = null;

async function once() {
  const { post } = await ask(POST, { slug });
  if (!post) {
    console.error(`no post with the slug "${slug}" — it may not be published yet`);
    process.exit(has("watch") ? 0 : 1);
  }
  const rank = post.featuredAt || post.createdAt ? await standing(post.id).catch(() => ({ place: 0, seen: 0 })) : { place: 0, seen: 0 };

  if (has("json")) {
    console.log(JSON.stringify({
      slug, id: post.id, name: post.name, tagline: post.tagline, url: post.url,
      votes: post.votesCount, comments: post.commentsCount, reviews: post.reviewsCount,
      featuredAt: post.featuredAt, createdAt: post.createdAt,
      rank: rank.place || null, of: rank.seen,
    }, null, 2));
    return;
  }

  const rate = before ? ` (+${post.votesCount - before.votes} since the last look)` : "";
  const line = (label, value) => console.log(`  ${label.padEnd(12)} ${value}`);
  console.log(`\n${post.name} — ${post.tagline}`);
  line("votes", `${post.votesCount}${rate}`);
  line("comments", post.commentsCount);
  if (post.reviewsCount) line("reviews", post.reviewsCount);
  if (rank.place) line("today", `#${rank.place} of ${rank.seen} counted`);
  line("live for", ago(post.featuredAt || post.createdAt));
  line("featured", post.featuredAt ? "yes" : "not yet");
  line("link", post.url);

  const fresh = post.comments.edges
    .map(({ node }) => node)
    .filter((c) => !before || !before.seen.has(c.id));
  if (fresh.length) {
    console.log(`\n  ${before ? "new comments" : "latest comments"}:`);
    for (const c of fresh.slice(0, 8)) {
      const body = c.body.replace(/\s+/g, " ").slice(0, 140);
      console.log(`   · ${c.user.name} (@${c.user.username}), ${ago(c.createdAt)} ago: ${body}`);
    }
    if (before) console.log("     ↑ answer these — replying fast is most of the game.");
  }
  before = { votes: post.votesCount, seen: new Set(post.comments.edges.map(({ node }) => node.id)) };
}

try {
  await once();
  if (has("watch")) {
    console.log(`\nwatching every ${every / 1000}s — Ctrl+C to stop`);
    setInterval(() => once().catch((e) => console.error(`  … ${e.message}`)), every);
  }
} catch (e) {
  console.error(e.message);
  process.exit(1);
}
