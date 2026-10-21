#!/usr/bin/env node

// Minimal streaming server that mimics React 18+ SSR with Suspense.
//
// Sends an HTML shell immediately, then resolves Suspense boundaries
// after artificial delays — exactly like a real Next.js app with
// async server components.
//
// Usage:
//   node server.mjs                          # starts on :3001
//   PORT=8080 node server.mjs                # custom port
//
// Then test with:
//   curl -N http://localhost:3001             # watch chunks arrive
//   curl -sN http://localhost:3001 | mdx2md --stream
//   # or open http://localhost:3001 in the streaming demo page (Fetch URL)

import http from "node:http";

const PORT = parseInt(process.env.PORT || "3001", 10);

// ── Page data (simulates async server component fetches) ─────────

const userData = {
  name: "Alice Chen",
  role: "Staff Engineer",
  avatar: "https://i.pravatar.cc/48?u=alice",
};

const stats = [
  { label: "Repositories", value: "47" },
  { label: "Pull Requests", value: "312" },
  { label: "Reviews", value: "1,204" },
  { label: "Commits (2026)", value: "2,847" },
];

const recentActivity = [
  { repo: "mdx2md", action: "merged PR #4", time: "2 hours ago" },
  { repo: "react-toolkit", action: "opened issue #89", time: "5 hours ago" },
  { repo: "mdx2md", action: "pushed to feat/streaming", time: "6 hours ago" },
  { repo: "design-system", action: "reviewed PR #201", time: "1 day ago" },
];

const notifications = [
  { type: "mention", text: "Bob mentioned you in icyJoseph/mdx2md#3" },
  { type: "review", text: "Review requested on react-toolkit#142" },
  { type: "ci", text: "CI passed on feat/streaming (6 checks)" },
];

// ── HTML fragments ───────────────────────────────────────────────

function shell() {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>Dashboard — Streaming Demo</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; background: #0d1117; color: #e6edf3; }
    h1 { font-size: 24px; } h2 { font-size: 18px; margin-top: 24px; }
    .card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; margin: 12px 0; }
    .loading { color: #8b949e; font-style: italic; }
    .stats { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; }
    .stat { text-align: center; } .stat .value { font-size: 28px; font-weight: bold; color: #58a6ff; }
    ul { list-style: none; padding: 0; } li { padding: 8px 0; border-bottom: 1px solid #30363d; }
    li:last-child { border-bottom: none; }
    .badge { display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 12px; background: #1f6feb33; color: #58a6ff; }
    footer { margin-top: 32px; padding-top: 16px; border-top: 1px solid #30363d; color: #8b949e; font-size: 13px; }
  </style>
</head>
<body>
  <header>
    <h1>Developer Dashboard</h1>
    <p>Welcome back! Here's your overview.</p>
  </header>
  <main>`;
}

function userProfileFallback() {
  return `<div class="card"><p class="loading">Loading profile...</p></div>`;
}

function userProfileResolved() {
  return `<div class="card">
  <h2>Profile</h2>
  <p><strong>${userData.name}</strong> — ${userData.role}</p>
  <p><img src="${userData.avatar}" alt="Avatar" width="48" height="48" /> <a href="https://github.com/icyjoseph">@icyJoseph</a></p>
</div>`;
}

function statsFallback() {
  return `<div class="card"><p class="loading">Crunching numbers...</p></div>`;
}

function statsResolved() {
  const items = stats
    .map((s) => `<div class="stat"><div class="value">${s.value}</div><div>${s.label}</div></div>`)
    .join("\n    ");
  return `<div class="card">
  <h2>Stats</h2>
  <div class="stats">
    ${items}
  </div>
</div>`;
}

function activityFallback() {
  return `<div class="card"><p class="loading">Fetching recent activity...</p></div>`;
}

function activityResolved() {
  const items = recentActivity
    .map(
      (a) =>
        `<li><strong>${a.repo}</strong>: ${a.action} <span class="badge">${a.time}</span></li>`,
    )
    .join("\n    ");
  return `<div class="card">
  <h2>Recent Activity</h2>
  <ul>
    ${items}
  </ul>
</div>`;
}

function notificationsFallback() {
  return `<div class="card"><p class="loading">Checking notifications...</p></div>`;
}

function notificationsResolved() {
  const items = notifications
    .map((n) => `<li><span class="badge">${n.type}</span> ${n.text}</li>`)
    .join("\n    ");
  return `<div class="card">
  <h2>Notifications</h2>
  <ul>
    ${items}
  </ul>
</div>`;
}

function footer() {
  return `
  </main>
  <footer>
    <p>Powered by <a href="https://github.com/icyjoseph/mdx2md">mdx2md</a> streaming converter.</p>
    <p>&copy; 2026 Streaming Demo</p>
  </footer>
</body>
</html>`;
}

// ── Boundary helpers (React SSR Suspense protocol) ───────────────

let boundaryCounter = 0;

function suspenseFallback(id, fallbackHtml) {
  return `<!--$?--><template id="B:${id}"></template>${fallbackHtml}<!--/$-->`;
}

function suspenseResolve(id, resolvedHtml) {
  return (
    `<div hidden id="S:${id}">${resolvedHtml}</div>` +
    `<script>$RC("B:${id}","S:${id}")</script>`
  );
}

// ── Request handler ──────────────────────────────────────────────

function handleRequest(req, res) {
  // Reset counter per request
  boundaryCounter = 0;

  // CORS headers for the playground demo page
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET");
  res.setHeader("Content-Type", "text/html; charset=utf-8");
  res.setHeader("Transfer-Encoding", "chunked");
  res.setHeader("X-Content-Type-Options", "nosniff");

  const profileId = boundaryCounter++;
  const statsId = boundaryCounter++;
  const activityId = boundaryCounter++;
  const notifsId = boundaryCounter++;

  // 1. Send shell immediately with Suspense fallbacks
  const shellHtml =
    shell() +
    suspenseFallback(profileId, userProfileFallback()) +
    suspenseFallback(statsId, statsFallback()) +
    suspenseFallback(activityId, activityFallback()) +
    suspenseFallback(notifsId, notificationsFallback()) +
    footer();

  res.write(shellHtml);

  // 2. Resolve boundaries out of order with realistic delays
  //    (profile is fast, stats next, then notifications, activity last)
  const resolutions = [
    { id: profileId, html: userProfileResolved, delay: 400 },
    { id: statsId, html: statsResolved, delay: 900 },
    { id: notifsId, html: notificationsResolved, delay: 1500 },
    { id: activityId, html: activityResolved, delay: 2200 },
  ];

  let resolved = 0;

  for (const r of resolutions) {
    setTimeout(() => {
      if (res.destroyed) return;
      res.write(suspenseResolve(r.id, r.html()));
      resolved++;
      if (resolved === resolutions.length) {
        res.end();
      }
    }, r.delay);
  }

  // Safety timeout
  setTimeout(() => {
    if (!res.destroyed) res.end();
  }, 5000);
}

// ── Start server ─────────────────────────────────────────────────

const server = http.createServer(handleRequest);

server.listen(PORT, () => {
  console.log(`\n  Streaming SSR server ready\n`);
  console.log(`  http://localhost:${PORT}\n`);
  console.log(`  Test with:`);
  console.log(`    curl -N http://localhost:${PORT}`);
  console.log(`    curl -sN http://localhost:${PORT} | mdx2md --stream`);
  console.log(`    # or use the playground: Fetch URL → http://localhost:${PORT}\n`);
});
