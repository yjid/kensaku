/*
 * Browser preview backend. Loaded ONLY when the app runs outside the Tauri
 * shell (plain `vite` / `vite preview`), so designers can see every screen
 * with representative content. Everything here is example data and never
 * ships inside the desktop app.
 *
  *   #setup=1     start on onboarding
  *   #q=invoice   open with a query
  *   #empty=1     home with no recent files
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
  f("/Users/mira/Design/Brand/kensaku-wordmark-final.svg", 12_400, 5 * 3600),
  f("/Users/mira/Pictures/Screenshots/Screenshot 2026-10-26 at 18.42.png", 1_820_000, day),
  f("/Users/mira/Code/kensaku/src-tauri/src/search/rank.rs", 18_900, day, "fn rerank(query: &Query, hits: &mut [Hit]) {", "Content"),
  f("/Users/mira/Pictures/Trips/Kyoto/IMG_4471.HEIC", 3_400_000, 3 * day),
  f("/Users/mira/Documents/Notes/Q4 planning.md", 6_200, 3 * day, "Ship onboarding, then the hide-to-tray memory target.", "Content"),
  f("/Users/mira/Movies/launch-teaser-v3.mp4", 148_000_000, 4 * day),
  f("/Users/mira/Documents/Resume/Mira Tan — Resume 2026.pdf", 96_000, 6 * day, "Principal engineer, search infrastructure.", "Semantic"),
  f("/Users/mira/Pictures/Moodboard/concrete-04.jpg", 2_100_000, 8 * day),
  f("/Users/mira/Documents/Finance/Expenses Oct.xlsx", 44_000, 9 * day),
  f("/Users/mira/Code/kensaku/README.md", 9_800, 10 * day, "Fast, light, private search for your files.", "Content"),
  f("/Users/mira/Pictures/Moodboard/glass-study-02.png", 1_640_000, 12 * day),
  f("/Users/mira/Documents/Talks/Search is a feeling.key", 22_000_000, 14 * day),
  f("/Users/mira/Design/Brand", 0, 15 * day),
  f("/Users/mira/Music/Field recordings/rain-on-tin.wav", 31_000_000, 20 * day),
  f("/Users/mira/Pictures/Trips/Kyoto/IMG_4502.HEIC", 3_100_000, 21 * day),
];

const THUMB_HUES: Record<string, [string, string]> = {
  png: ["#c7d2fe", "#312e81"], jpg: ["#e7e5e4", "#57534e"], heic: ["#fde68a", "#9a3412"],
  svg: ["#f5f5f4", "#18181b"], mp4: ["#a5f3fc", "#164e63"],
};
function thumb(path: string): string | null {
  const ext = path.split(".").pop()!.toLowerCase();
  const hue = THUMB_HUES[ext];
  if (!hue) return null;
  const seed = [...path].reduce((a, c) => a + c.charCodeAt(0), 0);
  const cx = 20 + (seed % 60), cy = 20 + ((seed >> 3) % 60);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"><defs><radialGradient id="g" cx="${cx}%" cy="${cy}%" r="90%"><stop offset="0" stop-color="${hue[0]}"/><stop offset="1" stop-color="${hue[1]}"/></radialGradient></defs><rect width="400" height="300" fill="url(#g)"/></svg>`;
  return "data:image/svg+xml;base64," + btoa(svg);
}

const config: Record<string, unknown> = {
  watched_dirs: ["/Users/mira/Documents", "/Users/mira/Pictures", "/Users/mira/Code"],
  excluded_dirs: ["node_modules", ".git"], excluded_extensions: [],
  shortcut: "Alt+Space", max_results: 200, is_setup_complete: params.get("setup") !== "1",
  semantic_enabled: true, ocr_enabled: true, ocr_engine: "tesseract", theme: "dark",
  saved_searches: [{ name: "Invoices", query: "invoice type:pdf" }, { name: "This week's screenshots", query: "screenshot modified:7d" }],
  pins: [], show_thumbnails: true, results_view: "grid", crawl_mode: "balanced",
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
    recent: params.get("empty") ? [] : FILES.slice(0, 12).map((r) => ({ path: r.path, filename: r.filename, size: r.file_size, modified: r.modified, is_dir: r.is_dir })),
    pins: [], saved_searches: config.saved_searches,
  }),
  get_indexing_status: () => indexStatus,
  search: (a) => search(a.query ?? ""),
  get_snippets: (a) => a.paths.map((p: string) => ({ path: p, snippet: FILES.find((r) => r.path === p)?.snippet ?? null })),
  get_thumbnail_data: (a) => thumb(a.path ?? ""),
  get_suggested_directories: () => ["/Users/mira/Documents", "/Users/mira/Desktop", "/Users/mira/Pictures", "/Users/mira/Code"],
  get_scope_suggestions: () => ["~/Documents", "~/Pictures", "~/Code"],
  get_ocr_status: () => ({ available: true, engine_present: true, accurate_compiled: true, languages: ["eng"] }),
  get_ocr_pack_status: () => ({ ready: false, downloading: false }),
  get_models_status: () => ({ ready: true }),
  get_autostart: () => true,
  get_file_details: (a) => {
    const r = FILES.find((x) => x.path === a.path);
    return { path: a.path, size: r?.file_size ?? 0, modified: r?.modified ?? now, created: (r?.modified ?? now) - 30 * day, width: /\.(png|jpg|heic)$/i.test(a.path) ? 4032 : null, height: /\.(png|jpg|heic)$/i.test(a.path) ? 3024 : null };
  },
  read_file_content: () => "Northwind Traders\nInvoice 2026-10\n\nDesign retainer, October    4,800.00\nPayment terms: net 14\n",
  find_duplicates: () => [],
  run_doctor: () => ({ checks: [] }),
  get_stats: () => ({ files: 48213, index_bytes: 212_000_000, memory_bytes: 38_000_000 }),
  agent_connect_status: () => [],
  "plugin:event|listen": () => 1,
  "plugin:event|unlisten": () => null,
};

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
