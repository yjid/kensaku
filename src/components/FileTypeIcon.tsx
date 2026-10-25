import React from "react";
import {
  FileText,
  FileCode,
  Folder,
  FileArchive,
  Image,
  File,
  Table,
  Presentation,
  AudioLines,
  Film,
} from "lucide-react";
import { classify, Category } from "../lib/categories";

/* File-type icon in a tinted square. Color by category:
   image=pink, doc=olive, code=blue, media=accent teal, folder/file/archive=neutral.
   The glyph stays full-strength; the square carries the tint. */

interface Meta {
  color: string;
  neutral?: boolean;
  glyph: (filename: string) => React.ReactNode;
}

const CATEGORY_META: Record<Category, Meta> = {
  folder: { color: "var(--fg-muted)", neutral: true, glyph: () => <Folder className="h-4 w-4" /> },
  image: { color: "var(--pink)", glyph: () => <Image className="h-4 w-4" /> },
  doc: {
    color: "var(--olive)",
    glyph: (fn) => {
      const ext = fn.split(".").pop()?.toLowerCase() || "";
      if (["xls", "xlsx", "csv", "tsv", "ods", "numbers"].includes(ext))
        return <Table className="h-4 w-4" />;
      if (["ppt", "pptx", "odp", "key"].includes(ext))
        return <Presentation className="h-4 w-4" />;
      return <FileText className="h-4 w-4" />;
    },
  },
  code: { color: "var(--blue)", glyph: () => <FileCode className="h-4 w-4" /> },
  media: {
    color: "var(--accent)",
    glyph: (fn) => {
      const ext = fn.split(".").pop()?.toLowerCase() || "";
      if (["mp4", "mov", "avi", "mkv", "webm", "m4v"].includes(ext))
        return <Film className="h-4 w-4" />;
      return <AudioLines className="h-4 w-4" />;
    },
  },
  archive: { color: "var(--fg-muted)", neutral: true, glyph: () => <FileArchive className="h-4 w-4" /> },
  file: { color: "var(--fg-muted)", neutral: true, glyph: () => <File className="h-4 w-4" /> },
};

export const FileTypeIcon: React.FC<{
  result: { is_dir: boolean; filename: string };
  size?: number;
}> = ({ result, size = 32 }) => {
  const cat = classify(result);
  const meta = CATEGORY_META[cat];
  return (
    <span
      aria-hidden="true"
      className="flex shrink-0 items-center justify-center rounded-lg"
      style={{
        width: size,
        height: size,
        color: meta.color,
        backgroundColor: meta.neutral
          ? "var(--bg-sunken)"
          : `color-mix(in srgb, ${meta.color} calc(var(--icon-tint-opacity) * 100%), transparent)`,
        border: "1px solid var(--border)",
      }}
    >
      {meta.glyph(result.filename)}
    </span>
  );
};
