#!/usr/bin/env node
/*
 * Records the demo clip by driving the browser preview with real keystrokes.
 *
 * The preview is the same React app the desktop shell loads, backed by
 * src/dev/mockTauri.ts, so what you see is the real UI responding to real key
 * events. It is not a capture of the packaged desktop build.
 *
 *   node scripts/demo/record.mjs [url] [outdir]
 *
 * Needs chromium on PATH and nothing else. Frames land in <outdir>/frames
 * with a manifest of wall-clock timings; scripts/demo/assemble.sh turns them
 * into assets/demo.mp4 and assets/demo.gif.
 */
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";

const URL_ = process.argv[2] ?? "http://127.0.0.1:8111/kshot/index.html";
const OUT = process.argv[3] ?? "/tmp/kensaku-demo";
/* The real window: src-tauri/src/main_window.rs, inner_size(920, 640). */
const W = 920;
const H = 640;
const SCALE = Number(process.env.SCALE ?? 1);
/* One capture round-trip costs ~100ms, so a serial loop tops out near 9fps
   and the typing reads as a slideshow. Keep N in flight instead. */
const LANES = Number(process.env.LANES ?? 3);
const QUERY = process.env.QUERY ?? "cuda out of memory";

const CHROME = process.env.CHROME ?? "chromium";
const PORT = 9300 + Math.floor(Math.random() * 600);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.error(`[demo] ${m}`);

fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(path.join(OUT, "frames"), { recursive: true });

const chrome = spawn(
  CHROME,
  [
    "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
    "--disable-dev-shm-usage", "--mute-audio",
    `--remote-debugging-port=${PORT}`,
    `--force-device-scale-factor=${SCALE}`,
    `--window-size=${W},${H}`,
    "--default-background-color=00000000",
    `--user-data-dir=${path.join(OUT, "chrome-profile")}`,
  ],
  { stdio: ["ignore", "ignore", "ignore"] },
);

/* Loopback has to bypass any proxy the environment exports. */
const getJson = (p) =>
  new Promise((resolve, reject) => {
    const req = http.request({ host: "127.0.0.1", port: PORT, path: p, agent: false }, (res) => {
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => {
        try { resolve(JSON.parse(b)); } catch (e) { reject(e); }
      });
    });
    req.on("error", reject);
    req.end();
  });

async function browserWs() {
  for (let i = 0; i < 80; i++) {
    try {
      const v = await getJson("/json/version");
      if (v?.webSocketDebuggerUrl) return v.webSocketDebuggerUrl;
    } catch { /* not up yet */ }
    await sleep(250);
  }
  throw new Error("chromium devtools never came up");
}

const ws = new WebSocket(await browserWs());
await new Promise((res, rej) => {
  ws.addEventListener("open", res, { once: true });
  ws.addEventListener("error", rej, { once: true });
});

let id = 0;
let session = null;
let stopLoop = false;
const pending = new Map();
const grabbed = [];

ws.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id === undefined) return;
  const p = pending.get(msg.id);
  if (!p) return;
  pending.delete(msg.id);
  msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result);
});

function send(method, params = {}, sid = session) {
  const myId = ++id;
  return new Promise((resolve, reject) => {
    pending.set(myId, { resolve, reject });
    ws.send(JSON.stringify(sid ? { id: myId, method, params, sessionId: sid } : { id: myId, method, params }));
  });
}

const { targetId } = await send("Target.createTarget", { url: URL_, newWindow: true }, null);
session = (await send("Target.attachToTarget", { targetId, flatten: true }, null)).sessionId;
await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: SCALE, mobile: false });
await sleep(2600); /* first paint, thumbnails, the stagger-in on the tiles */

const KEYS = Object.fromEntries(
  [..."abcdefghijklmnopqrstuvwxyz"].map((c) => [c, [`Key${c.toUpperCase()}`, c.charCodeAt(0) - 32]]),
);
KEYS[" "] = ["Space", 32];

async function typeChar(ch) {
  const [code, kc] = KEYS[ch] ?? ["Unidentified", 0];
  const base = { windowsVirtualKeyCode: kc, nativeVirtualKeyCode: kc, code, key: ch };
  await send("Input.dispatchKeyEvent", { ...base, type: "rawKeyDown" });
  await send("Input.dispatchKeyEvent", { ...base, type: "char", text: ch });
  await send("Input.dispatchKeyEvent", { ...base, type: "keyUp" });
}

async function press(key, code, kc) {
  const base = { windowsVirtualKeyCode: kc, nativeVirtualKeyCode: kc, code, key };
  await send("Input.dispatchKeyEvent", { ...base, type: "rawKeyDown" });
  await send("Input.dispatchKeyEvent", { ...base, type: "keyUp" });
}

/* Typed by a person: a little jitter, a longer beat between words. */
async function type(text) {
  for (const ch of text) {
    await typeChar(ch);
    await sleep(ch === " " ? 150 + Math.random() * 70 : 92 + Math.random() * 55);
  }
}

const lane = async () => {
  while (!stopLoop) {
    const t = Date.now();
    try {
      const r = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false, fromSurface: true });
      grabbed.push({ t, data: r.data });
    } catch { /* target busy, skip this one */ }
    const spent = Date.now() - t;
    if (spent < 22) await sleep(22 - spent);
  }
};
const capture = Promise.all(Array.from({ length: LANES }, lane));
log("recording");

/* ---- the clip ----------------------------------------------------------
   Space previews only after you have arrowed into the results (App.tsx),
   so this is the sequence a person actually performs. */
await sleep(1900);                       // home, recents, let it breathe
await type(QUERY);
await sleep(1700);                       // results land, read them
await press("ArrowDown", "ArrowDown", 40);
await sleep(420);
await press("ArrowUp", "ArrowUp", 38);   // back to the first hit
await sleep(620);
await press(" ", "Space", 32);           // quick look
await sleep(2600);                       // hold the payoff
/* ----------------------------------------------------------------------- */

stopLoop = true;
await capture;
grabbed.sort((a, b) => a.t - b.t);

const t0 = grabbed[0].t;
const manifest = grabbed.map((g, i) => {
  const file = path.join(OUT, "frames", `f${String(i).padStart(5, "0")}.png`);
  fs.writeFileSync(file, Buffer.from(g.data, "base64"));
  return { file, ms: g.t - t0, dur: (i + 1 < grabbed.length ? grabbed[i + 1].t : g.t + 80) - g.t };
});
fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 1));

const span = manifest.at(-1).ms / 1000;
log(`${manifest.length} frames over ${span.toFixed(1)}s (${(manifest.length / span).toFixed(1)} fps)`);
ws.close();
chrome.kill("SIGKILL");
process.exit(0);
