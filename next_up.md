# 🗺️ Kensaku — Next Up Roadmap

> **Target**: Sub-10ms universal search across filenames, code, full-text, PDFs, and images.
> **Distribution**: Single standalone binary (~18MB). Optional 1-click model download. Zero external dependencies.

---

## 📌 Current State (Completed in v0.1)

- [x] **Rust Engine**:
  - `nucleo-matcher`: In-memory path-aware fuzzy search (**< 1ms**).
  - `tantivy 0.26`: BM25 full-text keyword indexing across code and text files (**< 5ms**).
  - `jwalk 0.9`: Parallel multi-threaded filesystem crawler with pre-descent exclusion filtering.
  - `notify 8`: Live native OS event watcher (`inotify` on Linux) with batched commits.
- [x] **Desktop Shell (Tauri 2)**:
  - System tray with left-click toggle and menu.
  - Global shortcut (<kbd>Alt</kbd> + <kbd>Space</kbd>) that toggles a floating borderless spotlight bar.
  - Background daemon mode (window hide-on-close, index kept warm in RAM).
- [x] **Frontend (React 19 + Tailwind CSS v4)**:
  - First-launch setup wizard (directory picker + exclusion list + shortcut configuration).
  - Debounced search input (40ms) with search time counter in ms.
  - Keyboard navigation: <kbd>↑</kbd><kbd>↓</kbd> to select, <kbd>Enter</kbd> to open, <kbd>Shift</kbd>+<kbd>Enter</kbd> to reveal folder, <kbd>Esc</kbd> to dismiss.
  - Result cards with file icons, relative modified dates, formatted sizes, match badges, and text snippets.
  - In-app Settings modal for re-indexing and managing directories.
- [x] **Build & VCS**:
  - TypeScript compiles clean with 0 errors.
  - Rust backend compiles clean with 0 warnings.
  - Git repository initialized with initial commit.

---

## 🚀 The Next Phases (Kensaku 2.0 Roadmap)

```
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1: PDF & Document Parsing  (Pure Rust, Sub-ms text extraction)   │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 2: Vector Search Engine    (USearch HNSW + SimSIMD, < 0.5ms ANN) │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 3: AI Runtime & Models     (Quantized INT8, On-demand downloader)│
├────────────────────────────────────────────────────────────────────────┤
│ Phase 4: Multimodal Search       (MobileCLIP for Images, BGE for Docs) │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 5: Granular File Selection (Profiles, Drag & Drop, Tree Picker)  │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 6: Packaging & Launch      (Single binary, Benchmark GIF, Release)│
└────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 1: PDF & Document Text Parsing

* **Objective**: Make PDFs, rich documents, and office files searchable inside Kensaku with zero lag.
* **Key Tasks**:
  1. **Add `pdf-inspector` / `pdf-extract`** to `src-tauri/Cargo.toml`.
  2. Implement `extract_pdf_text(path: &Path) -> Option<String>` with layout and multi-column awareness.
  3. Chunk long PDFs into page-level or section-level passages so search results point to the exact page/context.
  4. Stream extracted PDF text into Tantivy's inverted index and the metadata store.
  5. Handle encrypted/scanned PDFs gracefully (flag as image for OCR or mark unindexed).

---

### Phase 2: In-Memory Vector Search Engine (`usearch`)

* **Objective**: Sub-0.5ms Approximate Nearest Neighbor (ANN) search over 100,000+ vector embeddings.
* **Key Tasks**:
  1. **Add `usearch = "0.2"`** to `src-tauri/Cargo.toml`.
  2. Configure HNSW index parameters:
     - Metric: `MetricKind::Cos` (Cosine similarity).
     - Quantization: `ScalarKind::I8` (INT8 quantization, keeping index size ~30MB for 100k items).
     - Dimensions: `384` for text, `512` for images.
  3. Implement persistent memory-mapped storage (`~/.config/kensaku/vectors/`).
  4. Add vector indexing API:
     - `add_doc_embedding(doc_id: u64, vector: &[f32])`
     - `search_knn(query_vector: &[f32], k: usize) -> Vec<(u64, f32)>`
  5. Benchmark latency: Guarantee vector retrieval in **< 0.5ms**.

---

### Phase 3: Single-Binary Model Downloader & Pure-Rust Inference

* **Objective**: Maintain a tiny ~18MB installer that can download optimized, quantized AI models on-demand.
* **Key Tasks**:
  1. **Integrate `fastembed` or `ort` (ONNX Runtime)**:
     - CPU AVX-512 / AVX2 / ARM NEON hardware acceleration.
     - Optional GPU acceleration (DirectML on Windows, CoreML on macOS, Vulkan/CUDA on Linux).
  2. **Model Selection (Tiny & Fast)**:
     - **Text Embedding**: `bge-micro-v2` or `all-MiniLM-L6-v2` (INT8, ~22 MB). Query embedding time: **~3.5ms**.
     - **Multimodal Vision**: `MobileCLIP-s0` or `SigLIP-base-int8` (~48 MB). Query text projection: **~4.0ms**.
  3. **In-App Model Downloader**:
     - Background download with progress bar in the UI.
     - Saved locally to `~/.config/kensaku/models/`.
     - Checksum verification for safety and integrity.
  4. **Idle-Time Background Worker**:
     - Embedding generation runs on a low-priority thread.
     - Only processes when user is idle (monitors CPU/keyboard activity so it never lags games or compilation).

---

### Phase 4: Multimodal Search (Images + Text in One Latent Space)

* **Objective**: Search images by description (*"screenshot of docker error"*, *"receipt from restaurant"*, *"sunset at beach"*) with zero slow OCR lag.
* **Key Tasks**:
  1. **Image Embedding Pipeline**:
     - Resize images to 224x224 during background idle indexing.
     - Pass through MobileCLIP image encoder to generate a 512-dim vector.
     - Store image vector in the `usearch` vision index.
  2. **Query Dispatcher Upgrade**:
     - When user types a query, embed it with the MobileCLIP text encoder.
     - Query both the text vector index and image vector index simultaneously.
  3. **Visual Result Cards**:
     - Image results show a thumbnail preview in the search list.
     - Double-click opens with system image viewer; Enter copies image or path.

---

### Phase 5: Granular File Selection & UX Polish

* **Objective**: Give power users total control over what gets indexed and what doesn't.
* **Key Tasks**:
  1. **Presets & Profiles**:
     - **Developer Mode**: Index code, configs, markdown, git repos; ignore `node_modules`, `target`, `build`, `.cache`, `venv`.
     - **Creative / Designer**: Index images (`.png`, `.jpg`, `.svg`, `.psd`), documents, videos; ignore system files.
     - **All-Round**: Entire `$HOME` with smart ignore list.
  2. **Folder Drag-and-Drop**:
     - Drag any folder directly into the search bar to immediately index that specific folder.
  3. **Directory Tree Inspector**:
     - Interactive checkbox tree in Settings showing indexed sizes and file counts per folder.
  4. **Sub-10ms Live Telemetry**:
     - Show exact query latency on the bottom bar: `⚡ 3.4ms (Lexical 0.8ms + Semantic 2.6ms)`.

---

### Phase 6: Packaging, Benchmarks & Launch Strategy

* **Objective**: Drive virality and stars on GitHub, X, and Hacker News.
* **Key Tasks**:
  1. **Automated Cross-Platform Releases via GitHub Actions**:
     - Linux: `.AppImage`, `.deb`, AUR package.
     - macOS: `.dmg` (Universal binary for Apple Silicon & Intel).
     - Windows: `.msi` and standalone `.exe`.
  2. **Benchmark Suite**:
     - Standalone script comparing Kensaku vs. Spotlight vs. Windows Search on a 100,000-file repository.
  3. **15-Second Demo GIF**:
     - Show <kbd>Alt</kbd>+<kbd>Space</kbd> $\to$ typing `"screenshot of python syntax error"` $\to$ instant image result $\to$ opening it in under 1 second.
  4. **The Launch Post Framing**:
     - *"OS search takes 5 seconds to fail. We built Kensaku in Rust to search 100k files + images + text in under 10ms."*

---

## 🛠️ Verification & Test Plan

When implementing each phase, test with these benchmarks:

| Test Case | Acceptance Criteria |
|---|---|
| **Fuzzy Filename Query** | Latency $< 1\text{ms}$ on 50,000 files |
| **Full-Text BM25 Query** | Latency $< 5\text{ms}$ on 10,000 text files |
| **PDF Text Extraction** | Extract 50-page PDF in $< 50\text{ms}$ |
| **Semantic Text Search** | Latency $< 8\text{ms}$ (embedding + HNSW KNN) |
| **Image Search by Description** | Return matching image within $< 8\text{ms}$ |
| **Idle Resource Usage** | $< 35\text{MB}$ RAM, $0\%$ CPU while idle in tray |
| **Binary Size** | $< 25\text{MB}$ standalone executable |

---

*Ready for your review. Whenever you're ready, let me know which phase or feature you want to tackle first.*
