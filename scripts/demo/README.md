# Demo clip

`assets/demo.gif` and `assets/demo.mp4` are generated from the browser
preview, not from the packaged desktop build. The preview runs the same
React app the Tauri shell loads, backed by the example data in
`src/dev/mockTauri.ts`, and the clip is recorded by driving it with real
key events over the Chrome DevTools Protocol. The window size matches the
real one (920x640, `src-tauri/src/main_window.rs`).

Re-record after a UI change:

```sh
bun install
npx vite build --base=/kshot/ --outDir /tmp/srv/kshot
(cd /tmp/srv && python3 -m http.server 8111 &)

node scripts/demo/record.mjs http://127.0.0.1:8111/kshot/index.html /tmp/kensaku-demo
GIF_FPS=17 FPS=24 scripts/demo/assemble.sh /tmp/kensaku-demo assets
```

Needs `chromium` and `ffmpeg` on PATH. Nothing else.

| var | default | what it does |
|---|---|---|
| `QUERY` | `cuda out of memory` | what gets typed |
| `LANES` | `3` | captures in flight; raises the frame rate |
| `SCALE` | `1` | device pixel ratio (2 doubles size and halves the rate) |
| `GIF_FPS` | `14` | gif frame rate |
| `GIF_W` | `920` | gif width |

A real screen recording of the packaged app is still better if you have
one. This exists so the repo always has a current clip that re-renders
when the UI changes, instead of a stale one nobody wants to re-shoot.
