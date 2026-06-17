import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { useTauriEvent } from "../hooks/useTauriEvent";
import type { Citation, IndexHealth, Memory, RetrievalMode, TokenPayload, UnsupportedClaim } from "../types";

interface DisplayMessage {
  role: "user" | "assistant" | "system";
  content: string;
  citations?: Citation[];
  uncited_warning?: boolean;
  unsupported_claims?: UnsupportedClaim[];
}

export function ChatPage() {
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [useKnowledgeBase, setUseKnowledgeBase] = useState(false);
  const [retrievalMode, setRetrievalMode] = useState<RetrievalMode>("balanced");
  const [indexHealth, setIndexHealth] = useState<IndexHealth | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [extracting, setExtracting] = useState(false);
  const [extractNotice, setExtractNotice] = useState<string | null>(null);
  const [showMemoriesUsed, setShowMemoriesUsed] = useState(false);
  const [lastMemories, setLastMemories] = useState<Memory[]>([]);

  useEffect(() => {
    Promise.all([api.getConfig(), api.indexHealth()])
      .then(([config, health]) => {
        setRetrievalMode(config.default_retrieval_mode as RetrievalMode);
        setIndexHealth(health);
      })
      .catch(() => {});
  }, []);

  useTauriEvent<TokenPayload>(
    "token",
    useCallback((payload) => {
      setMessages((prev) => {
        const next = [...prev];
        const last = next[next.length - 1];

        if (payload.token.startsWith("[error:")) {
          next.push({ role: "system", content: payload.token });
          return next;
        }

        if (!payload.finished) {
          if (last?.role === "assistant") {
            next[next.length - 1] = {
              ...last,
              content: last.content + payload.token,
            };
          } else {
            next.push({ role: "assistant", content: payload.token });
          }
          return next;
        }

        // finished === true
        if (payload.token) {
          // Single-shot responses (e.g. direct memory answers) send text with finished=true.
          if (last?.role === "assistant") {
            next[next.length - 1] = {
              ...last,
              content: last.content + payload.token,
              citations: payload.citations ?? last.citations,
              uncited_warning: payload.uncited_warning ?? last.uncited_warning,
              unsupported_claims: payload.unsupported_claims ?? last.unsupported_claims,
            };
          } else {
            next.push({
              role: "assistant",
              content: payload.token,
              citations: payload.citations,
              uncited_warning: payload.uncited_warning,
              unsupported_claims: payload.unsupported_claims,
            });
          }
        } else if (last?.role === "assistant") {
          next[next.length - 1] = {
            ...last,
            citations: payload.citations ?? last.citations,
            uncited_warning: payload.uncited_warning,
            unsupported_claims: payload.unsupported_claims,
          };
        }
        return next;
      });
      if (payload.finished) {
        setStreaming(false);
        api.getLastInjectedMemories().then(setLastMemories).catch(() => {});
      }
    }, []),
  );

  const send = async () => {
    const text = input.trim();
    if (!text || streaming) return;
    setError(null);
    setInput("");
    setMessages((m) => [...m, { role: "user", content: text }]);
    setStreaming(true);

    try {
      if (useKnowledgeBase) {
        await api.ragStreamStart(text, retrievalMode);
      } else {
        await api.chatStreamStart(text);
      }
    } catch (e) {
      setStreaming(false);
      setError(String(e));
    }
  };

  const extractMemories = async () => {
    setExtracting(true);
    setExtractNotice(null);
    setError(null);
    try {
      const created = await api.extractMemoryCandidates();
      setExtractNotice(
        created.length > 0
          ? `Proposed ${created.length} candidate(s). Review them on the Memory page.`
          : "No new durable memories found in this conversation.",
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setExtracting(false);
    }
  };

  const exportChat = async () => {
    try {
      const data = await api.saveConversation();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `ashkorix-chat-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(String(e));
    }
  };

  const ragDisabled =
    indexHealth !== null && (!indexHealth.indexed || !indexHealth.embedding_loaded);

  return (
    <>
      <div className="panel">
        <div className="row" style={{ marginBottom: "0.75rem" }}>
          <label className="muted">
            <input
              type="checkbox"
              checked={useKnowledgeBase}
              onChange={(e) => setUseKnowledgeBase(e.target.checked)}
              disabled={ragDisabled}
            />{" "}
            Use knowledge base
          </label>
          {useKnowledgeBase && (
            <select
              className="select"
              value={retrievalMode}
              onChange={(e) => setRetrievalMode(e.target.value as RetrievalMode)}
            >
              <option value="fast">fast</option>
              <option value="balanced">balanced</option>
              <option value="thorough">thorough</option>
              <option value="deep">deep</option>
            </select>
          )}
          {ragDisabled && (
            <span className="muted">
              {indexHealth?.message ?? "Index not ready — check Documents and Settings"}
            </span>
          )}
        </div>

        <div className="row" style={{ marginBottom: "0.75rem" }}>
          <button
            type="button"
            className="btn"
            onClick={() => api.clearConversation().then(() => setMessages([]))}
          >
            Clear
          </button>
          <button type="button" className="btn" onClick={exportChat}>
            Export JSON
          </button>
          <button
            type="button"
            className="btn"
            onClick={extractMemories}
            disabled={extracting || streaming || messages.length === 0}
          >
            {extracting ? "Extracting…" : "Extract memories"}
          </button>
          <label className="muted">
            <input
              type="checkbox"
              checked={showMemoriesUsed}
              onChange={(e) => setShowMemoriesUsed(e.target.checked)}
            />{" "}
            Show memories used
          </label>
          {streaming && (
            <button type="button" className="btn btn-danger" onClick={() => api.cancelGeneration()}>
              Stop
            </button>
          )}
        </div>

        {extractNotice && <p className="success">{extractNotice}</p>}

        {showMemoriesUsed && lastMemories.length > 0 && (
          <div className="panel nested" style={{ marginBottom: "0.75rem", fontSize: "0.85rem" }}>
            <strong>Memories used last turn</strong>
            <ul>
              {lastMemories.map((m) => (
                <li key={m.id}>
                  [{m.memory_type}] {m.content}
                </li>
              ))}
            </ul>
          </div>
        )}

        <div className="chat-log">
          {messages.length === 0 && (
            <p className="muted">Load a model on the Models page, then start chatting.</p>
          )}
          {messages.map((m, i) => (
            <div key={i}>
              <div className={`chat-bubble ${m.role}`}>{m.content}</div>
              {m.citations && m.citations.length > 0 && (
                <div className="panel" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
                  <strong>Sources</strong>
                  {m.citations.map((c) => (
                    <div key={c.source_number} style={{ marginTop: "0.35rem" }}>
                      [{c.source_number}] {c.original_filename}
                      <p className="muted" style={{ margin: "0.2rem 0 0" }}>
                        {c.chunk_preview}
                      </p>
                    </div>
                  ))}
                </div>
              )}
              {m.uncited_warning && (
                <p className="error" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
                  Warning: this answer has no source citations.
                </p>
              )}
              {m.unsupported_claims && m.unsupported_claims.length > 0 && (
                <div className="panel nested" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
                  <strong className="error">Unsupported claims</strong>
                  <p className="muted" style={{ margin: "0.25rem 0 0.5rem" }}>
                    These statements are not supported by the cited source text.
                  </p>
                  <ul>
                    {m.unsupported_claims.map((u, j) => (
                      <li key={j}>
                        {u.sentence}
                        {u.cited_source != null && ` [Source ${u.cited_source}]`}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ))}
        </div>

        <div className="row">
          <textarea
            className="textarea"
            style={{ flex: 1, minHeight: "72px" }}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
            placeholder="Message… (Enter to send)"
            disabled={streaming}
          />
          <button
            type="button"
            className="btn btn-primary"
            onClick={send}
            disabled={streaming || !input.trim()}
          >
            {streaming ? "Sending…" : "Send"}
          </button>
        </div>
        {error && <p className="error">{error}</p>}
      </div>
    </>
  );
}
