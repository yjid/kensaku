// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "../types";
import { classify, matchesToken } from "../lib/categories";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("../lib/thumbnails", () => ({ isThumbImage: () => false }));

import { PreviewDialog } from "./PreviewDialog";

const cleaned = "title: Synthetic metadata\nVisible label\nlet value = 42;";

function result(filename: string): SearchResult {
  return { path: `/synthetic/${filename}`, filename, snippet: null, score: 1,
    file_size: 100, modified: 0, is_dir: false, match_type: "Content" };
}

describe("Markdown Quick Look", () => {
  it.each(["note.md", "note.markdown", "NOTE.MARKDOWN"])("shows extracted content for %s", async (filename) => {
    vi.mocked(invoke).mockReset().mockResolvedValue(cleaned);
    const file = result(filename);
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    try {
      await act(async () => {
        root.render(<PreviewDialog result={file} onOpen={() => {}} onClose={() => {}} />);
      });
      expect(invoke).toHaveBeenCalledExactlyOnceWith("read_file_content", { path: file.path });
      expect(host.querySelector("pre")?.textContent).toBe(cleaned);
      expect(classify(file)).toBe("doc");
      expect(matchesToken(file, "type:doc")).toBe(true);
    } finally {
      act(() => root.unmount());
      host.remove();
    }
  });

  it("does not read an unsupported binary as text", async () => {
    vi.mocked(invoke).mockReset();
    const host = document.createElement("div");
    const root = createRoot(host);
    try {
      await act(async () => {
        root.render(<PreviewDialog result={result("file.bin")} onOpen={() => {}} onClose={() => {}} />);
      });
      expect(invoke).not.toHaveBeenCalled();
      expect(host.querySelector("pre")).toBeNull();
    } finally {
      act(() => root.unmount());
    }
  });
});
