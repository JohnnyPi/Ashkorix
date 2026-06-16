import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { useTauriEvent } from "../hooks/useTauriEvent";
import type { Citation, IndexHealth, RagAnswer, RetrievalMode, TokenPayload } from "../types";

interface DisplayMessage {
  role: "user" | "assistant" | "system";
  content: string;
  citations?: Citation[];
}

export function ChatPage() {
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [useKnowledgeBase, setUseKnowledgeBase] = useState(false);
  const [retrievalMode, setRetrievalMode] = useState<RetrievalMode>("balanced");
  const [indexHealth, setIndexHealth] = useState<IndexHealth | null>(null);
  const [error, setError] = useState<string | null>(null);

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
        if (last?.role === "assistant" && !payload.finished) {
          next[next.length - 1] = {
            ...last,
            content: last.content + payload.token,
          };
        } else if (payload.token.startsWith("[error:")) {
          next.push({ role: "system", content: payload.token });
        } else if (!payload.finished) {
          next.push({ role: "assistant", content: payload.token });
        }
        return next;
      });
      if (payload.finished) setStreaming(false);
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
        const answer: RagAnswer = await api.ask(text, retrievalMode);
        setMessages((m) => [
          ...m,
          {
            role: "assistant",
            content: answer.text,
            citations: answer.citations,
          },
        ]);
        if (answer.unsupported_claims.length > 0) {
          setMessages((m) => [
            ...m,
            {
              role: "system",
              content: `Verification: ${answer.unsupported_claims.length} claim(s) may lack source support.`,
            },
          ]);
        }
        setStreaming(false);
      } else {
        await api.chatStreamStart(text);
      }
    } catch (e) {
      setStreaming(false);
      setError(String(e));
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
              <option value="corpus-map">corpus map</option>
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
          {streaming && !useKnowledgeBase && (
            <button type="button" className="btn btn-danger" onClick={() => api.cancelGeneration()}>
              Stop
            </button>
          )}
        </div>

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
            {streaming ? (useKnowledgeBase ? "Thinking…" : "Sending…") : "Send"}
          </button>
        </div>
        {error && <p className="error">{error}</p>}
      </div>
    </>
  );
}
