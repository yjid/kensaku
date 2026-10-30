/*
 * Browser preview backend. Loaded ONLY when the app runs outside the Tauri
 * shell (plain `vite` / `vite preview`), so designers can see every screen
 * with representative content. Everything here is example data and never
 * ships inside the desktop app.
 *
  *   #setup=1     start on onboarding
  *   #q=invoice   open with a query
  *   #empty=1     home with no recent files
  *   #view=list   results as a list instead of the tile grid
  *   #theme=light start in the light theme
 */
import { classify } from "../lib/categories";

const params = new URLSearchParams(location.hash.slice(1) || location.search);
const now = Math.floor(Date.now() / 1000);
const day = 86400;

type R = {
  path: string; filename: string; snippet: string | null; score: number;
  file_size: number; modified: number; is_dir: boolean; match_type: string;
};
const f = (path: string, size: number, ago: number, snippet: string | null = null, match = "FileName"): R => ({
  path, filename: path.split("/").pop()!, snippet, score: 1, file_size: size,
  modified: now - ago, is_dir: !/\.[a-z0-9]+$/i.test(path), match_type: match,
});

const FILES: R[] = [
  f("/Users/mira/Documents/Contracts/Northwind invoice 2026-10.pdf", 284_000, 2 * 3600, "Invoice total due on receipt. Payment terms net 14.", "Both"),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-10-22 at 11.07.png", 740_000, 26 * 3600, "PAID · invoice #88241 · Northwind Traders", "Content"),
  f("/Users/mira/Documents/Finance/invoice-tracker.xlsx", 61_000, 2 * day, "Oct invoice sent, awaiting payment", "Both"),
  f("/Users/mira/Documents/Contracts/Acme invoice 2026-09.pdf", 191_000, 36 * day, "Invoice 2026-09. Design retainer, September.", "Both"),
  f("/Users/mira/Documents/Notes/Chasing the Acme invoice.md", 3_100, 5 * day, "They said the invoice was paid on the 14th.", "Content"),
  f("/Users/mira/Documents/Contracts/Northwind invoice 2026-08.pdf", 203_000, 68 * day, "Invoice 2026-08. Design retainer, August.", "Both"),
  f("/Users/mira/Downloads/invoice_receipt_8841.pdf", 48_000, 11 * day, "Receipt for invoice 8841. Thank you.", "Both"),
  f("/Users/mira/Documents/Finance/Expenses Oct.xlsx", 44_000, 9 * day, "Row 14: Northwind invoice, reimbursable", "Content"),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-09-30 at 16.55.png", 612_000, 29 * day, "Overdue: invoice 2026-08 · 31 days", "Content"),
  f("/Users/mira/Documents/Templates/invoice-template.docx", 28_000, 54 * day, "[Client name] — invoice [number]", "Both"),
  f("/Users/mira/Code/kensaku/docs/billing-notes.md", 4_400, 17 * day, "Stripe sends the invoice; we never store a card.", "Content"),
  /* The screenshot story: text that only exists inside the pixels. */
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-07-14 094231.png", 1_240_000, 106 * day, "RuntimeError: CUDA out of memory. Tried to allocate 2.00 GiB", "Content"),
  f("/Users/mira/Code/train/logs/run-0714.log", 2_900_000, 106 * day, "torch.cuda.OutOfMemoryError: CUDA out of memory. Tried 2.00 GiB", "Content"),
  f("/Users/mira/Documents/Notes/Training journal.md", 7_800, 104 * day, "Second CUDA out of memory crash today. Dropped the batch to 16.", "Content"),
  f("/Users/mira/Code/train/finetune.py", 11_200, 103 * day, "# batch_size=16: anything higher and CUDA runs out of memory", "Content"),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-07-16 221540.png", 980_000, 104 * day, "CUDA out of memory again at step 3100, batch 24", "Content"),
  f("/Users/mira/Code/train/logs/run-0716.log", 3_400_000, 104 * day, "step 3100: CUDA out of memory. 22.1 GiB already allocated", "Content"),
  f("/Users/mira/Documents/Reading/pytorch-cuda-out-of-memory.pdf", 118_000, 101 * day, "Avoiding CUDA out of memory: gradient checkpointing", "Both"),
  f("/Users/mira/Code/train/profile/mem-0714.json", 262_000, 106 * day, "peak_reserved_gib: 23.4 · cuda out of memory at step 240", "Content"),
  f("/Users/mira/Documents/Notes/Slack - ml-infra export.txt", 31_000, 105 * day, "mira: still CUDA out of memory on the 7B, even at batch 8", "Content"),
  f("/Users/mira/Code/train/README.md", 5_200, 100 * day, "If CUDA goes out of memory, halve the batch and retry.", "Content"),
  f("/Users/mira/Code/train/logs/run-0718.log", 2_100_000, 102 * day, "retry 3: CUDA out of memory, falling back to batch 8", "Content"),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-07-18 085512.png", 822_000, 102 * day, "CUDA out of memory. See docs for memory management", "Content"),
  f("/Users/mira/Documents/Notes/GPU memory checklist.md", 4_900, 100 * day, "Before you blame CUDA: out of memory is usually the batch", "Content"),
  f("/Users/mira/Code/train/configs/batch-sweep.yaml", 1_400, 101 * day, "# 32 and 24 both hit CUDA out of memory. 16 is the ceiling.", "Content"),
  f("/Users/mira/Documents/Reading/activation-checkpointing.pdf", 240_000, 98 * day, "Trading compute for CUDA out of memory headroom", "Both"),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-07-14 101902.png", 486_000, 106 * day, "nvidia-smi · CUDA Version 12.4 · 23.7 GiB · 1 process", "Content"),
  f("/Users/mira/Code/train/setup-cuda.sh", 1_900, 110 * day, "export CUDA_VISIBLE_DEVICES=0", "Both"),
  f("/Users/mira/Documents/Notes/CUDA install notes.md", 5_600, 112 * day, "cuda 12.4 with cudnn 9, driver 550. Reboot after.", "Both"),
  f("/Users/mira/Code/train/benchmarks.ipynb", 94_000, 99 * day, "cuda vs mps throughput, 7B at bf16", "Content"),
  f("/Users/mira/Downloads/cuda_12.4.0_550.54.14_linux.run", 4_100_000, 113 * day),
  f("/Users/mira/Design/Brand/kensaku-wordmark-final.svg", 12_400, 5 * 3600),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-10-26 at 18.42.png", 1_820_000, day),
  f("/Users/mira/Code/kensaku/src-tauri/src/search/rank.rs", 18_900, day, "fn rerank(query: &Query, hits: &mut [Hit]) {", "Content"),
  f("/Users/mira/Pictures/Trips/Kyoto/IMG_4471.HEIC", 3_400_000, 3 * day),
  f("/Users/mira/Documents/Notes/Q4 planning.md", 6_200, 3 * day, "Ship onboarding, then the hide-to-tray memory target.", "Content"),
  f("/Users/mira/Movies/launch-teaser-v3.mp4", 148_000_000, 4 * day),
  f("/Users/mira/Documents/Resume/Mira Tan — Resume 2026.pdf", 96_000, 6 * day, "Principal engineer, search infrastructure.", "Semantic"),
  f("/Users/mira/Pictures/Moodboard/concrete-04.jpg", 2_100_000, 8 * day),
  f("/Users/mira/Code/kensaku/README.md", 9_800, 10 * day, "Fast, light, private search for your files.", "Content"),
  f("/Users/mira/Pictures/Moodboard/glass-study-02.png", 1_640_000, 12 * day),
  f("/Users/mira/Documents/Talks/Search is a feeling.key", 22_000_000, 14 * day),
  f("/Users/mira/Design/Brand", 0, 15 * day),
  f("/Users/mira/Music/Field recordings/rain-on-tin.wav", 31_000_000, 20 * day),
  f("/Users/mira/Pictures/Trips/Kyoto/IMG_4502.HEIC", 3_100_000, 21 * day),
];

/* Flat sample pictures: sky, ground, one form. No gradients. */
const SCENES: [string, string, string][] = [
  ["#d9dde3", "#7d8590", "#f4f1ea"],
  ["#e8e4dc", "#5d5a52", "#c9b79c"],
  ["#c9d3dc", "#2f3a45", "#e9e6df"],
  ["#ece9e4", "#a39e94", "#3a3a3a"],
  ["#d4d8d0", "#56604f", "#f2efe8"],
];
/* The SVG goes through btoa(), which is Latin1-only, and the data URL is
   parsed as UTF-8. Anything outside ASCII either throws or decodes to a bad
   byte and the whole image fails to parse, so fold to ASCII first. */
const esc = (s: string) =>
  s.replace(/[\u2018\u2019]/g, "'").replace(/[\u201c\u201d]/g, '"').replace(/[\u2013\u2014]/g, "-")
   .replace(/[^\x20-\x7e]/g, "-")
   .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/* A screenshot should look like a screenshot. These sit in /Screenshots/, so
   draw the thing they are a capture of: a terminal, with the file's own OCR'd
   line as the one that matters. 16/10 so the tile never crops it. Flat, two
   greys and one muted red. */
const MONO_CH = 7.82; /* 13px ui-monospace advance width */
function wrap(text: string, width: number): string[] {
  const max = Math.floor(width / MONO_CH);
  const out: string[] = [];
  let line = "";
  for (const word of text.split(" ")) {
    if (line && (line + " " + word).length > max) { out.push(line); line = word; }
    else line = line ? line + " " + word : word;
  }
  if (line) out.push(line);
  return out.slice(0, 2);
}
function screenshotThumb(path: string): string {
  const line = FILES.find((r) => r.path === path)?.snippet ?? "";
  const row = (t: string, y: number, fill: string) =>
    `<text x="18" y="${y}" font-family="ui-monospace,monospace" font-size="13" fill="${fill}">${esc(t)}</text>`;
  const bar = (y: number, w: number) => `<rect x="18" y="${y}" width="${w}" height="7" rx="2" fill="#262b31"/>`;
  /* A capture of a terminal and a capture of a document do not look alike.
     Pick by what the extracted text reads like, so the sample picture is a
     picture of the thing its own text came from. */
  const terminal = /cuda|torch|epoch|traceback|error|step \d/i.test(line);
  const body = terminal
    ? ["$ python finetune.py --batch 32", "loading checkpoint: 7B-bf16", "epoch 1/3  step 240/5100"]
        .map((t, i) => row(t, 52 + i * 24, "#6a7078"))
        .join("")
    : [220, 300, 160].map((w, i) => bar(42 + i * 24, w)).join("");
  const lines = wrap(line, 364);
  const errRows = lines.map((t, i) => row(t, 152 + i * 24, terminal ? "#d2817b" : "#c7cbd1")).join("");
  const tail = terminal ? row("$ _", 152 + lines.length * 24, "#6a7078") : [180, 240].map((w, i) => bar(152 + lines.length * 24 + i * 24, w)).join("");
  return (
    "data:image/svg+xml;base64," +
    btoa(
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 250"><rect width="400" height="250" fill="#121417"/>` +
        `<rect width="400" height="24" fill="#1b1e23"/>` +
        `<circle cx="18" cy="12" r="3.5" fill="#3a3f47"/><circle cx="33" cy="12" r="3.5" fill="#3a3f47"/><circle cx="48" cy="12" r="3.5" fill="#3a3f47"/>` +
        body + errRows + tail + `</svg>`,
    )
  );
}

function thumb(path: string): string | null {
  if (!/\.(png|jpe?g|heic|svg|mp4)$/i.test(path)) return null;
  if (path.includes("/Screenshots/")) return screenshotThumb(path);
  const seed = [...path].reduce((a, c) => a + c.charCodeAt(0), 0);
  const [sky, ground, form] = SCENES[seed % SCENES.length];
  const horizon = 150 + (seed % 70);
  const cx = 80 + (seed % 240), r = 26 + (seed % 30);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"><rect width="400" height="300" fill="${sky}"/><circle cx="${cx}" cy="${horizon - 50}" r="${r}" fill="${form}"/><rect y="${horizon}" width="400" height="${300 - horizon}" fill="${ground}"/></svg>`;
  return "data:image/svg+xml;base64," + btoa(svg);
}

const config: Record<string, unknown> = {
  watched_dirs: ["/Users/mira/Documents", "/Users/mira/Pictures", "/Users/mira/Code"],
  excluded_dirs: ["node_modules", ".git"], excluded_extensions: [],
  shortcut: "Alt+Space", max_results: 200, is_setup_complete: params.get("setup") !== "1",
  semantic_enabled: true, ocr_enabled: true, ocr_engine: "tesseract",
  theme: params.get("theme") === "light" ? "light" : "dark",
  saved_searches: [{ name: "Invoices", query: "invoice type:pdf" }, { name: "This week's screenshots", query: "screenshot modified:7d" }],
  pins: [], show_thumbnails: true,
  results_view: params.get("view") === "list" ? "list" : "grid",
  crawl_mode: "balanced",
};

function search(query: string) {
  const q = query.toLowerCase().replace(/\w+:\S+/g, "").trim();
  const words = q.split(/\s+/).filter(Boolean);
  const results = FILES.filter((r) => words.every((w) => (r.path + " " + (r.snippet ?? "")).toLowerCase().includes(w)));
  const list = results.length || words.length ? results : FILES;
  return {
    results: list, seq: 0, active_filters: [], hints: [], search_time_ms: 4,
    total: list.length, mode: "ranked", interpretation: [],
    facets: {
      all: list.length,
      types: list.reduce<Record<string, number>>((acc, r) => {
        const c = classify({ is_dir: r.is_dir, filename: r.filename });
        acc[c] = (acc[c] ?? 0) + 1;
        if (!r.is_dir) acc.file = (acc.file ?? 0) + 1;
        return acc;
      }, {}),
      languages: {}, places: {}, time: {}, size: {},
      recent: list.filter((r) => now - r.modified < 7 * day).length,
    },
  };
}

const FILE_BODIES: Record<string, string> = {
  "/Users/mira/Documents/Contracts/Northwind invoice 2026-10.pdf":
    "Northwind Traders\nInvoice 2026-10\n\nDesign retainer, October    4,800.00\nPayment terms: net 14\n",
  "/Users/mira/Code/train/logs/run-0714.log":
    "epoch 1/3  step 238/5100  loss 2.104  lr 1.0e-4\n" +
    "epoch 1/3  step 239/5100  loss 2.098  lr 1.0e-4\n" +
    "epoch 1/3  step 240/5100  loss 2.091  lr 1.0e-4\n" +
    "Traceback (most recent call last):\n" +
    "  File \"finetune.py\", line 212, in train\n" +
    "    loss.backward()\n" +
    "torch.cuda.OutOfMemoryError: CUDA out of memory. Tried to allocate\n" +
    "2.00 GiB (GPU 0; 23.69 GiB total capacity; 21.88 GiB already\n" +
    "allocated; 1.44 GiB free; 22.03 GiB reserved in total by PyTorch)\n",
  "/Users/mira/Documents/Notes/Training journal.md":
    "# Training journal\n\n## Jul 14\nSecond CUDA out of memory crash today.\n" +
    "Dropped the batch to 16 and it held through the night.\n\n" +
    "## Jul 16\nStill out of memory at step 3100 with batch 24. Keeping 16.\n",
};

const indexStatus = { is_indexing: false, files_indexed: 48213, content_indexed: 31877, current_status: "Up to date", semantic_ready: true, phase: "idle" };

const handlers: Record<string, (a: any) => unknown> = {
  is_setup_complete: () => config.is_setup_complete,
  is_setup_configured: () => false,
  get_reset_pending: () => false,
  last_query: () => params.get("q") ?? "",
  remember_query: () => null,
  get_config: () => structuredClone(config),
  save_config: (a) => Object.assign(config, a.config),
  get_home_data: () => ({
    recent: params.get("empty") ? [] : FILES.slice(0, 12).map((r) => ({ path: r.path, filename: r.filename, size: r.file_size, modified: r.modified, is_dir: r.is_dir, last_open: 0, opens: 0 })),
    pins: [], saved_searches: config.saved_searches,
  }),
  get_indexing_status: () => indexStatus,
  search: (a) => search(a.query ?? ""),
  get_snippets: (a) => a.paths.map((p: string) => ({ path: p, snippet: FILES.find((r) => r.path === p)?.snippet ?? null })),
  get_thumbnail_data: (a) => thumb(a.path ?? ""),
  get_suggested_directories: () => ["/Users/mira/Documents", "/Users/mira/Desktop", "/Users/mira/Pictures", "/Users/mira/Code"],
  get_scope_suggestions: () => ["~/Documents", "~/Pictures", "~/Code"],
  get_ocr_status: () => ({ available: true, engine_present: true, accurate_compiled: true, accurate_available: true, accurate_problem: null, active_engine: "tesseract", engine: "tesseract", languages: ["eng"] }),
  get_ocr_pack_status: () => ({ ready: false, downloading: false, dir: "~/.config/kensaku/ocr", artifacts: [] }),
  get_indra_pack_status: () => ({ ready: false, downloading: false, dir: "~/.config/kensaku/ocr-indra", artifacts: [] }),
  get_models_status: () => ({ ready: true }),
  get_autostart: () => true,
  get_file_details: (a) => {
    const r = FILES.find((x) => x.path === a.path);
    return { path: a.path, size: r?.file_size ?? 0, modified: r?.modified ?? now, created: (r?.modified ?? now) - 30 * day, width: /\.(png|jpg|heic)$/i.test(a.path) ? 4032 : null, height: /\.(png|jpg|heic)$/i.test(a.path) ? 3024 : null };
  },
  read_file_content: (a) => {
    const body = FILE_BODIES[a.path as string];
    if (body) return body;
    const snippet = FILES.find((r) => r.path === a.path)?.snippet;
    return snippet ? `${snippet}\n` : "";
  },
  find_duplicates: () => [],
  run_doctor: () => ({ checks: [], summary: { ok: 0, warn: 0, fail: 0 } }),
  get_stats: () => ({ files: 48213, index_bytes: 212_000_000, memory_bytes: 38_000_000 }),
  agent_connect_status: () => [],
  "plugin:event|listen": () => 1,
  "plugin:event|unlisten": () => null,
};

function note(msg: string) {
  const el = document.createElement("div");
  el.textContent = msg;
  el.className = "elevated overlay-in";
  Object.assign(el.style, {
    position: "fixed", left: "50%", bottom: "56px", transform: "translateX(-50%)", zIndex: "999",
    padding: "8px 16px", borderRadius: "999px", fontSize: "13px", fontWeight: "500", color: "var(--fg)",
    pointerEvents: "none", whiteSpace: "nowrap",
  });
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 1800);
}
handlers.open_file = (a) => note(`Preview: would open ${a.path.split("/").pop()}`);
handlers.open_folder = (a) => note(`Preview: would reveal ${a.path.split("/").pop()} in its folder`);
handlers.hide_window = () => null;
handlers.validate_directory = (args: { path: string }) => args.path;

let cbId = 0;
(window as any).__TAURI_INTERNALS__ = {
  invoke: async (cmd: string, args: any) => {
    const h = handlers[cmd];
    if (!h) return null;
    return h(args ?? {});
  },
  transformCallback: () => ++cbId,
  unregisterCallback: () => {},
  convertFileSrc: (p: string) => p,
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
};
(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
