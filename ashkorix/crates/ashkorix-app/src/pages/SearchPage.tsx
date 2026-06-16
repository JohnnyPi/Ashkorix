import { useEffect, useState } from "react";
import { api } from "../api";
import type { CorpusMapResult, IndexHealth, RankedChunk, RetrievalMode } from "../types";

const MODES: { id: RetrievalMode; label: string }[] = [
  { id: "fast", label: "Fast" },
  { id: "balanced", label: "Balanced" },
  { id: "thorough", label: "Thorough" },
  { id: "deep", label: "Deep" },
  { id: "corpus-map", label: "Corpus Map" },
];

function ChunkList({ chunks }: { chunks: RankedChunk[] }) {
  if (chunks.length === 0) return null;
  return (
    <div className="panel">
      <h3>Retrieved chunks ({chunks.length})</h3>
      {chunks.map((r, i) => (
        <div
          key={`${r.chunk.id}-${i}`}
          style={{
            marginBottom: "0.85rem",
            paddingBottom: "0.85rem",
            borderBottom: "1px solid #252830",
          }}
        >
          <div className="row">
            <span className="badge">#{r.source_number ?? i + 1}</span>
            <span className="muted">rrf {r.score.toFixed(4)}</span>
            {r.rerank_score != null && (
              <span className="muted">rerank {r.rerank_score.toFixed(4)}</span>
            )}
            <span className="badge">{r.source_type}</span>
            <span className="muted">{r.chunk.source_filename}</span>
            {r.chunk.page_number != null && (
              <span className="muted">p.{r.chunk.page_number}</span>
            )}
          </div>
          {r.chunk.heading_path && (
            <p className="muted" style={{ margin: "0.35rem 0 0", fontSize: "0.8rem" }}>
              {r.chunk.heading_path}
            </p>
          )}
          {r.expanded_context && (
            <pre
              style={{
                margin: "0.35rem 0 0",
                fontSize: "0.75rem",
                color: "#8b949e",
                whiteSpace: "pre-wrap",
              }}
            >
              {r.expanded_context}
            </pre>
          )}
          <pre
            style={{
              margin: "0.5rem 0 0",
              fontSize: "0.85rem",
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              background: "#0f1115",
              padding: "0.65rem",
              borderRadius: 8,
              maxHeight: 240,
              overflow: "auto",
            }}
          >
            {r.chunk.text}
          </pre>
        </div>
      ))}
    </div>
  );
}

function CorpusMapPanel({ map }: { map: CorpusMapResult }) {
  return (
    <div className="panel">
      <h3>Corpus Map</h3>
      {map.themes.length > 0 && (
        <>
          <h4>Themes</h4>
          {map.themes.map((t) => (
            <div key={t.document_id} style={{ marginBottom: "0.75rem" }}>
              <strong>{t.title}</strong>
              <p className="muted">{t.summary}</p>
            </div>
          ))}
        </>
      )}
      {map.entities.length > 0 && (
        <>
          <h4>Top entities</h4>
          <div className="row">
            {map.entities.slice(0, 15).map((e) => (
              <span key={e.value} className="badge">
                {e.value} ({e.count})
              </span>
            ))}
          </div>
        </>
      )}
      {map.sections.length > 0 && (
        <>
          <h4>Sections</h4>
          <ul className="muted">
            {map.sections.slice(0, 20).map((s) => (
              <li key={`${s.document_id}-${s.heading_path}`}>{s.heading_path}</li>
            ))}
          </ul>
        </>
      )}
      <ChunkList chunks={map.related_chunks} />
    </div>
  );
}

export function SearchPage() {
  const [indexHealth, setIndexHealth] = useState<IndexHealth | null>(null);
  const [mode, setMode] = useState<RetrievalMode>("balanced");
  const [query, setQuery] = useState("");
  const [chunks, setChunks] = useState<RankedChunk[]>([]);
  const [corpusMap, setCorpusMap] = useState<CorpusMapResult | null>(null);
  const [queryVariants, setQueryVariants] = useState<string[]>([]);
  const [sectionFilter, setSectionFilter] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getConfig()
      .then((c) => setMode(c.default_retrieval_mode as RetrievalMode))
      .catch(() => {});
    api.indexHealth().then(setIndexHealth).catch(() => setIndexHealth(null));
  }, []);

  const run = async () => {
    if (!query.trim()) return;
    setBusy(true);
    setError(null);
    setChunks([]);
    setCorpusMap(null);
    setQueryVariants([]);
    try {
      if (mode === "corpus-map") {
        const answer = await api.ask(query.trim(), mode);
        setQueryVariants(answer.query_variants);
        setCorpusMap(answer.corpus_map);
      } else {
        const filters = {
          document_ids: [],
          file_types: [],
          page_min: null,
          page_max: null,
          section_prefix: sectionFilter.trim() || null,
          entity_match: null,
        };
        setChunks(await api.retrieve(query.trim(), mode, [], filters));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="panel">
        <h3>Retrieval inspector</h3>
        <p className="muted" style={{ marginTop: 0 }}>
          Hybrid lexical + vector search with query planning, reranking, and context expansion.
        </p>

        {indexHealth && (
          <p className="muted" style={{ marginTop: 0 }}>
            {indexHealth.indexed ? (
              <>
                <span className="badge badge-ok">indexed</span> {indexHealth.chunk_count} chunks ·{" "}
                {indexHealth.vector_count} vectors · {indexHealth.lexical_count} lexical docs
              </>
            ) : (
              <span className="badge badge-warn">not indexed — {indexHealth.message}</span>
            )}
          </p>
        )}

        <div className="row" style={{ marginBottom: "0.75rem", flexWrap: "wrap", gap: "0.5rem" }}>
          <label className="muted">
            Mode{" "}
            <select
              className="select"
              value={mode}
              onChange={(e) => setMode(e.target.value as RetrievalMode)}
            >
              {MODES.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </select>
          </label>
          {mode !== "corpus-map" && (
            <label className="muted">
              Section prefix{" "}
              <input
                className="input"
                value={sectionFilter}
                onChange={(e) => setSectionFilter(e.target.value)}
                placeholder="Chapter 2"
              />
            </label>
          )}
        </div>

        <textarea
          className="textarea"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Query to test retrieval…"
        />
        <div className="row" style={{ marginTop: "0.75rem" }}>
          <button
            type="button"
            className="btn btn-primary"
            onClick={run}
            disabled={busy || !query.trim()}
          >
            {busy ? "Retrieving…" : mode === "corpus-map" ? "Map corpus" : "Retrieve"}
          </button>
        </div>
        {queryVariants.length > 0 && (
          <p className="muted" style={{ marginTop: "0.5rem" }}>
            Query variants: {queryVariants.join(" · ")}
          </p>
        )}
        {error && <p className="error">{error}</p>}
      </div>

      {corpusMap && <CorpusMapPanel map={corpusMap} />}
      <ChunkList chunks={chunks} />
    </>
  );
}
