import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText, Loader2, MessageCircleQuestion, Sparkles, X } from "lucide-react";
import type { AskResponse } from "../types";

/**
 * `/ask` — answers a question strictly from the local index.
 *
 * The backend contract is "no retrieval, no answer": the model only ever
 * sees numbered snippets from real indexed files, and the response carries
 * the sources so citations can be shown (and clicked). Cloud endpoints are
 * never used from the GUI unless the user enabled one in settings AND
 * confirms here; loopback providers (Ollama, LM Studio) run as-is.
 */
export function AskPanel({
  open,
  onClose,
  onOpenFile,
}: {
  open: boolean;
  onClose: () => void;
  onOpenFile: (path: string) => void;
}) {
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState<AskResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cloudConfirm, setCloudConfirm] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      inputRef.current?.focus();
    } else {
      // Reset on close so the next ask starts clean.
      setAnswer(null);
      setError(null);
      setBusy(false);
    }
  }, [open]);

  const run = useCallback(async () => {
    const trimmed = question.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    setError(null);
    setAnswer(null);
    try {
      const response = await invoke<AskResponse>("ask_question", {
        question: trimmed,
        allowCloud: cloudConfirm,
      });
      setAnswer(response);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [question, busy, cloudConfirm]);

  if (!open) return null;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="flex max-h-[80vh] w-full max-w-[640px] flex-col overflow-hidden rounded-[16px] border border-line bg-elevated shadow-lg">
        <div className="flex items-center gap-2 border-b border-line px-4 py-3">
          <MessageCircleQuestion className="h-4 w-4 text-accent" />
          <span className="text-[13px] font-semibold text-fg">Ask your files</span>
          <span className="ml-auto font-mono text-[11px] text-subtle">/ask</span>
          <button
            type="button"
            onClick={onClose}
            className="flex h-6 w-6 items-center justify-center rounded text-subtle transition-colors hover:bg-sunken hover:text-fg"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        <form
          className="flex items-center gap-2 border-b border-line px-4 py-3"
          onSubmit={(e) => {
            e.preventDefault();
            void run();
          }}
        >
          <input
            ref={inputRef}
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            placeholder="What do your files say about…"
            className="min-w-0 flex-1 bg-transparent text-[13px] text-fg outline-none placeholder:text-subtle"
          />
          <button
            type="submit"
            disabled={busy || !question.trim()}
            className="flex h-7 shrink-0 items-center gap-1.5 rounded-[8px] border border-line px-3 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-3 w-3 animate-spin" /> : <Sparkles className="h-3 w-3" />}
            Ask
          </button>
        </form>

        <div className="flex items-center gap-2 border-t border-line px-4 py-2">
          <input
            id="ask-cloud-consent"
            type="checkbox"
            checked={cloudConfirm}
            onChange={(e) => setCloudConfirm(e.target.checked)}
            className="h-3 w-3"
          />
          <label htmlFor="ask-cloud-consent" className="text-[11px] text-subtle">
            Allow a non-local (cloud) endpoint for this answer
          </label>
        </div>

        {(cloudConfirm || answer?.cloud) && (
          <div className="border-t border-warning/40 bg-warning/10 px-4 py-2 text-[11px] leading-relaxed text-warning">
            Cloud endpoint{answer?.endpoint ? ` (${answer.endpoint})` : ""}: this question and
            the retrieved snippets leave this device.
          </div>
        )}

        <div className="no-scrollbar min-h-[120px] flex-1 overflow-y-auto px-4 py-3">
          {error && (
            <p className="text-[12px] leading-relaxed text-warning">
              {error}
              <span className="mt-1 block text-[11px] text-muted">
                Configure a local model in Settings → Agents → Local AI.
              </span>
            </p>
          )}

          {!error && !answer && !busy && (
            <p className="text-[12px] leading-relaxed text-muted">
              Answers come only from your indexed files, with path and line citations. If nothing
              matches, there is no answer — the model never guesses from its own knowledge.
            </p>
          )}

          {answer && (
            <>
              <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-fg">
                {answer.answer}
              </p>
              {answer.sources.length > 0 && (
                <div className="mt-3 border-t border-line pt-2">
                  <p className="pb-1 text-[11px] text-muted">
                    Sources {answer.answered ? "(cited first)" : ""}
                  </p>
                  {answer.sources.map((source) => (
                    <button
                      key={source.index}
                      type="button"
                      onClick={() => onOpenFile(source.path)}
                      className="flex w-full items-baseline gap-2 rounded px-1 py-1 text-left transition-colors hover:bg-sunken"
                    >
                      <span
                        className={`shrink-0 font-mono text-[11px] ${
                          source.cited ? "text-accent" : "text-subtle"
                        }`}
                      >
                        [{source.index}]
                      </span>
                      <FileText className="h-3 w-3 shrink-0 text-subtle" />
                      <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-fg">
                        {source.path}
                      </span>
                      <span className="shrink-0 font-mono text-[11px] text-subtle">
                        {source.line_start}–{source.line_end}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
