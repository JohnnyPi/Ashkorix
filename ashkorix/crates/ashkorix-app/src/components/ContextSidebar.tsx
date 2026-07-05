import { useEffect, useState, type ReactNode } from "react";
import { Link, useLocation, useSearchParams } from "react-router-dom";
import { api } from "../api";
import { useTelemetry } from "../hooks/useTelemetry";
import { useChatSession } from "../context/ChatSessionContext";
import { resolvePageId } from "../routes";
import { cn } from "../lib/cn";
import type {
  GraphOverview,
  IndexHealth,
  ModelInfo,
  SessionSummary,
  SleepStatus,
} from "../types";

function SectionLabel({ children }: { children: ReactNode }) {
  return <div className="mb-2.5 font-mono text-[9px] tracking-[0.16em] text-ink-muted/65">{children}</div>;
}

function Divider() {
  return <div className="my-4 h-px bg-border-subtle/80" />;
}

function StatCard({ label, value, accent }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="reactive-surface rounded-[5px] border border-border-subtle bg-bg/35 px-2.5 py-2">
      <div className="mb-1 font-mono text-[8px] tracking-[0.1em] text-ink-muted/75">{label}</div>
      <div className={cn("font-mono text-[17px] font-semibold", accent ? "text-phosphor" : "text-ink")}>{value}</div>
    </div>
  );
}

function KvRow({ k, v, ok }: { k: string; v: string; ok?: boolean }) {
  return (
    <div className="flex justify-between gap-2 font-mono text-[11px]">
      <span className="text-ink-muted">{k}</span>
      <span className={cn("truncate text-right", ok ? "text-success" : "text-ink")}>{v}</span>
    </div>
  );
}

const fmt = (n?: number | null) => (n == null ? "—" : n.toLocaleString());

function formatSessionDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function GraphContext({ overview }: { overview: GraphOverview | null }) {
  const { model } = useTelemetry();
  const [params, setParams] = useSearchParams();
  const [expanded, setExpanded] = useState(false);
  const [filter, setFilter] = useState("");

  const s = overview?.summary;
  const selectedDoc = params.get("doc");
  const selectedEntity = params.get("entity");

  const q = filter.trim().toLowerCase();
  const documents =
    overview?.documents.filter(
      (d) => !q || d.label.toLowerCase().includes(q) || d.id.toLowerCase().includes(q),
    ) ?? [];
  const entities =
    overview?.top_entities.filter(
      (e) => !q || e.value.toLowerCase().includes(q) || e.kind.toLowerCase().includes(q),
    ) ?? [];

  return (
    <>
      <SectionLabel>CORPUS</SectionLabel>
      <div className="mb-3.5 grid grid-cols-2 gap-2">
        <StatCard label="DOCS" value={fmt(s?.document_count)} />
        <StatCard label="CHUNKS" value={fmt(s?.chunk_count)} />
        <StatCard label="ENTITIES" value={fmt(s?.entity_count)} />
        <StatCard label="EDGES" value={fmt(s?.relation_count)} accent />
      </div>

      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
        className={cn(
          "reactive-control mb-3.5 flex w-full items-center justify-between rounded-[5px] border bg-surface-raised px-3 py-2.5 font-display text-[12px] font-medium",
          expanded
            ? "border-phosphor/40 text-phosphor"
            : "border-border-strong text-ink-muted hover:border-phosphor hover:text-phosphor",
        )}
      >
        Filter documents &amp; entities
        <span className="font-mono text-[13px] leading-none">{expanded ? "−" : "+"}</span>
      </button>

      {expanded && (
        <div className="mb-3.5 flex flex-col gap-3">
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter…"
            className="reactive-control w-full rounded border border-border-strong bg-bg/50 px-2.5 py-1.5 font-mono text-[11px] text-ink outline-none placeholder:text-ink-muted/50 focus:border-phosphor focus:bg-hud-bg"
          />
          <div className="quiet-scrollbar max-h-[40vh] overflow-y-auto pr-1">
            <div className="mb-1.5 font-mono text-[9px] tracking-[0.1em] text-ink-muted">DOCUMENTS</div>
            <div className="flex flex-col gap-1">
              {documents.map((d) => (
                <button
                  key={d.id}
                  type="button"
                  onClick={() => setParams({ doc: d.id })}
                  className={cn(
                    "reactive-control rounded border px-2 py-1.5 text-left",
                    selectedDoc === d.id
                      ? "border-phosphor/50 bg-accent-soft text-ink"
                      : "border-transparent text-ink-muted hover:border-border hover:bg-white/5",
                  )}
                >
                  <div className="truncate text-[12px] font-medium">{d.label}</div>
                  <div className="mt-0.5 font-mono text-[10px] text-ink-muted">
                    {d.chunk_count} chunks · {d.entity_count} entities
                  </div>
                </button>
              ))}
              {documents.length === 0 && (
                <div className="font-mono text-[11px] text-ink-muted">No documents match.</div>
              )}
            </div>

            <div className="mb-1.5 mt-3 font-mono text-[9px] tracking-[0.1em] text-ink-muted">TOP ENTITIES</div>
            <div className="flex flex-wrap gap-1.5">
              {entities.map((e) => (
                <button
                  key={e.id}
                  type="button"
                  onClick={() => setParams({ entity: e.id })}
                  className={cn(
                    "reactive-control rounded-md border px-2 py-0.5 font-mono text-[10px]",
                    selectedEntity === e.id
                      ? "border-phosphor/50 bg-accent-soft text-phosphor"
                      : "border-border text-ink-muted hover:border-border-strong hover:text-ink",
                  )}
                >
                  {e.value} ({e.mention_count})
                </button>
              ))}
              {entities.length === 0 && (
                <div className="font-mono text-[11px] text-ink-muted">No entities match.</div>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="flex gap-3 font-mono text-[11px] text-ink-muted">
        <span>
          <span className="text-ink">{fmt(s?.chunk_count)}</span> chunks
        </span>
        <span>
          <span className="text-ink">{fmt(s?.entity_count)}</span> entities
        </span>
      </div>
      <Divider />
      <SectionLabel>MODEL</SectionLabel>
      <div className="flex flex-col gap-2.5">
        <KvRow k="file" v={model.modelFile ?? "—"} />
        <KvRow k="ctx" v={String(model.ctx)} />
        <KvRow k="status" v={model.status} />
      </div>
    </>
  );
}

function ChatContext() {
  const { ragMeta } = useTelemetry();
  const { session, actions, sessionsRevision, refreshSessions } = useChatSession();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .listSessions()
      .then((s) => {
        if (alive) {
          setSessions(s);
          setHistoryError(null);
        }
      })
      .catch((e) => alive && setHistoryError(String(e)));
    return () => {
      alive = false;
    };
  }, [sessionsRevision]);

  const handleDelete = async (id: string) => {
    setDeleting(true);
    try {
      await api.deleteSession(id);
      setConfirmId(null);
      setHistoryError(null);
      refreshSessions();
    } catch (e) {
      setHistoryError(String(e));
    } finally {
      setDeleting(false);
    }
  };

  const hasTurns = session.turns > 0;
  const turnLabel = `${session.turns} ${session.turns === 1 ? "turn" : "turns"}`;
  const stateLabel = session.streaming ? "generating" : hasTurns ? "active" : "empty";

  return (
    <>
      <SectionLabel>SESSION</SectionLabel>
      <div className="rounded-[5px] border border-phosphor/30 bg-accent-soft px-2.5 py-2">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              session.streaming ? "bg-phosphor status-pulse" : "bg-status-off",
            )}
            aria-hidden
          />
          <span className="text-[13px] font-semibold text-[#cfe9e6]">Current session</span>
        </div>
        <div className="mt-1 font-mono text-[10px] text-ink-muted">
          {stateLabel} · {turnLabel}
        </div>
      </div>
      <div className="mt-2 flex gap-2">
        <button
          type="button"
          onClick={() => void actions.newSession()}
          disabled={session.streaming || !hasTurns}
          className="reactive-control flex-1 rounded-[5px] border border-border-strong bg-surface-raised py-2 font-display text-[12px] font-medium text-ink-muted hover:border-phosphor hover:bg-accent-soft hover:text-phosphor disabled:cursor-not-allowed disabled:opacity-50"
        >
          New chat
        </button>
        <button
          type="button"
          onClick={() => void actions.exportChat()}
          disabled={!hasTurns}
          className="reactive-control rounded-[5px] border border-border-strong bg-surface-raised px-3 py-2 font-display text-[12px] font-medium text-ink-muted hover:border-phosphor hover:bg-accent-soft hover:text-phosphor disabled:cursor-not-allowed disabled:opacity-50"
        >
          Export
        </button>
      </div>

      <Divider />
      <SectionLabel>HISTORY</SectionLabel>
      {historyError && (
        <p role="alert" className="mb-2 font-mono text-[10px] leading-relaxed text-rose">
          {historyError}
        </p>
      )}
      {sessions.length === 0 ? (
        <p className="font-mono text-[11px] leading-relaxed text-ink-muted/70">
          No saved sessions yet. Starting a new chat archives the current one.
        </p>
      ) : (
        <div className="quiet-scrollbar flex max-h-[38vh] flex-col gap-1 overflow-y-auto pr-1">
          {sessions.map((s) => {
            const isActive = s.session_id === session.activeSessionId;
            const confirming = confirmId === s.session_id;
            return (
              <div
                key={s.session_id}
                className={cn(
                  "reactive-surface rounded-[5px] border",
                  isActive
                    ? "border-phosphor/50 bg-accent-soft"
                    : "border-transparent hover:border-border hover:bg-white/5",
                )}
              >
                {confirming ? (
                  <div className="flex items-center justify-between gap-2 px-2.5 py-2">
                    <span className="font-mono text-[10px] text-ink-muted">Delete session?</span>
                    <span className="flex shrink-0 gap-1.5">
                      <button
                        type="button"
                        onClick={() => void handleDelete(s.session_id)}
                        disabled={deleting}
                        className="reactive-control rounded border border-rose/40 px-2 py-0.5 font-display text-[11px] text-rose hover:bg-rose/10 disabled:opacity-50"
                      >
                        Delete
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirmId(null)}
                        className="reactive-control rounded border border-border-strong px-2 py-0.5 font-display text-[11px] text-ink-muted hover:bg-white/5 hover:text-ink"
                      >
                        Cancel
                      </button>
                    </span>
                  </div>
                ) : (
                  <div className="flex items-stretch">
                    <button
                      type="button"
                      onClick={() => void actions.loadSession(s.session_id)}
                      disabled={session.streaming}
                      className="min-w-0 flex-1 px-2.5 py-2 text-left disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <div className={cn("truncate text-[12px] font-medium", isActive ? "text-ink" : "text-ink-muted")}>
                        {s.title}
                      </div>
                      <div className="mt-0.5 font-mono text-[10px] text-ink-muted">
                        {formatSessionDate(s.created_at)} · {s.turns} {s.turns === 1 ? "turn" : "turns"}
                      </div>
                    </button>
                    <button
                      type="button"
                      aria-label={isActive ? "Active session cannot be deleted" : "Delete session"}
                      title={isActive ? "Start a new chat before deleting this session" : "Delete session"}
                      onClick={() => setConfirmId(s.session_id)}
                      disabled={session.streaming || isActive}
                      className="reactive-control shrink-0 px-2 text-base leading-none text-ink-muted/50 hover:text-rose disabled:opacity-30"
                    >
                      ✕
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      <Divider />
      <SectionLabel>RETRIEVAL</SectionLabel>
      <div className="flex flex-col gap-2.5">
        <KvRow k="collection" v={ragMeta.collection} />
        <KvRow k="rerank" v={ragMeta.rerankActive ? "active" : "off"} ok={ragMeta.rerankActive} />
        <KvRow k="index" v={ragMeta.indexed ? "ready" : "pending"} ok={ragMeta.indexed} />
      </div>
    </>
  );
}

function MemoryContext({
  reviewCount,
  trashCount,
  store,
}: {
  reviewCount: number | null;
  trashCount: number | null;
  store: string;
}) {
  const { memory } = useTelemetry();
  const tabs = [
    { id: "active", label: "Active", count: memory.active, dot: "bg-phosphor", countColor: "text-phosphor" },
    { id: "review", label: "Review", count: reviewCount, dot: "bg-phosphor-alt", countColor: "text-phosphor-alt" },
    { id: "trash", label: "Trash", count: trashCount, dot: "bg-status-off", countColor: "text-ink-muted" },
  ];
  return (
    <>
      <SectionLabel>MEMORY STORE</SectionLabel>
      <div className="flex flex-col gap-2">
        {tabs.map((t) => {
          const isActive = store === t.id;
          return (
            <Link
              key={t.id}
              to={`/memory?store=${t.id}`}
              className={cn(
                "reactive-control flex items-center justify-between rounded-[5px] border px-3 py-2.5",
                isActive ? "border-phosphor/40 bg-accent-soft" : "border-border hover:border-border-strong",
              )}
            >
              <span className="flex items-center gap-2">
                <span className={cn("h-1.5 w-1.5 rounded-full", t.dot)} aria-hidden />
                <span className="font-display text-sm font-semibold text-ink">{t.label}</span>
              </span>
              <span className={cn("font-mono text-base", t.countColor)}>{t.count ?? "—"}</span>
            </Link>
          );
        })}
      </div>
      <Divider />
      <div className="font-mono text-[11px] leading-relaxed text-ink-muted">
        scope
        <br />
        <span className="text-phosphor">project:{memory.scope}</span>
      </div>
    </>
  );
}

function SidebarJump({ href, label, detail }: { href: string; label: string; detail?: string }) {
  return (
    <a
      href={href}
      className="reactive-control group flex items-center justify-between gap-2 rounded border border-transparent px-2 py-1.5 text-[11px] text-ink-muted hover:border-phosphor/30 hover:bg-accent-soft hover:text-ink"
    >
      <span>{label}</span>
      <span className="font-mono text-[9px] text-ink-muted/60 group-hover:text-phosphor">
        {detail ?? "→"}
      </span>
    </a>
  );
}

function ModelsContext({ info, count }: { info: ModelInfo | null; count: number | null }) {
  const { connection, model } = useTelemetry();
  return (
    <>
      <SectionLabel>MODEL RUNTIME</SectionLabel>
      <div className="rounded-[5px] border border-phosphor/30 bg-accent-soft px-2.5 py-2">
        <div className="truncate text-[12px] font-semibold text-ink">
          {model.modelFile ?? info?.filename ?? "No model loaded"}
        </div>
        <div className="mt-1 font-mono text-[9px] text-ink-muted">{model.status}</div>
      </div>
      <div className="mt-3 flex flex-col gap-2">
        <KvRow k="discovered" v={count == null ? "—" : String(count)} />
        <KvRow k="context" v={info ? String(info.n_ctx) : "—"} />
        <KvRow
          k="compute"
          v={connection.cudaAvailable ? connection.cudaDevice ?? "CUDA" : "CPU"}
          ok={connection.cudaAvailable}
        />
      </div>
      <Divider />
      <SectionLabel>WORKFLOW</SectionLabel>
      <SidebarJump href="#available-models" label="Choose a model" />
      <SidebarJump href="#loaded-model" label="Runtime details" />
      <Link className="mt-2 block px-2 text-[10px] text-phosphor hover:underline" to="/settings#models-retrieval">
        Retrieval model settings →
      </Link>
    </>
  );
}

function DocumentsContext({ count, health }: { count: number | null; health: IndexHealth | null }) {
  return (
    <>
      <SectionLabel>CORPUS STATUS</SectionLabel>
      <div className="grid grid-cols-2 gap-2">
        <StatCard label="DOCS" value={count == null ? "—" : String(count)} />
        <StatCard label="CHUNKS" value={fmt(health?.chunk_count)} accent />
      </div>
      <div className="mt-3 flex flex-col gap-2">
        <KvRow k="index" v={health?.indexed ? "ready" : "pending"} ok={health?.indexed} />
        <KvRow k="embed" v={health?.embedding_loaded ? "loaded" : "offline"} ok={health?.embedding_loaded} />
        <KvRow k="vectors" v={fmt(health?.vector_count)} />
      </div>
      <Divider />
      <SectionLabel>WORKFLOW</SectionLabel>
      <SidebarJump href="#knowledge-base" label="Import & index" detail="1" />
      <SidebarJump href="#document-library" label="Browse documents" detail="2" />
      <Link className="mt-2 block px-2 text-[10px] text-phosphor hover:underline" to="/search">
        Test retrieval →
      </Link>
    </>
  );
}

function SearchContext({ health }: { health: IndexHealth | null }) {
  const { ragMeta } = useTelemetry();
  return (
    <>
      <SectionLabel>RETRIEVAL</SectionLabel>
      <div className="rounded-[5px] border border-phosphor/30 bg-accent-soft px-2.5 py-2">
        <div className="font-mono text-[9px] tracking-wider text-ink-muted">INDEX</div>
        <div className="mt-1 text-[12px] font-semibold text-ink">
          {health?.indexed ? `${health.chunk_count} searchable chunks` : "Index not ready"}
        </div>
      </div>
      <div className="mt-3 flex flex-col gap-2">
        <KvRow k="collection" v={ragMeta.collection} />
        <KvRow k="rerank" v={ragMeta.rerankActive ? "active" : "off"} ok={ragMeta.rerankActive} />
        <KvRow k="embedding" v={health?.embedding_loaded ? "on" : "off"} ok={health?.embedding_loaded} />
      </div>
      <Divider />
      <SectionLabel>INSPECT</SectionLabel>
      <SidebarJump href="#retrieval-controls" label="Query controls" />
      <SidebarJump href="#search-results" label="Ranked output" />
      <p className="mt-3 px-2 font-mono text-[9px] leading-relaxed text-ink-muted/70">
        Start broad, then add a section prefix only when the result set is noisy.
      </p>
    </>
  );
}

function SleepContext({ status, runs }: { status: SleepStatus | null; runs: number | null }) {
  const threshold = status
    ? Math.min(100, Math.round((status.adapter_worthy_samples / Math.max(1, status.min_new_samples)) * 100))
    : 0;
  return (
    <>
      <SectionLabel>CURATION</SectionLabel>
      <div className="grid grid-cols-2 gap-2">
        <StatCard label="QUEUED" value={fmt(status?.unprocessed_transcripts)} accent />
        <StatCard label="RUNS" value={runs == null ? "—" : String(runs)} />
      </div>
      <div className="mt-3 flex flex-col gap-2">
        <KvRow k="threshold" v={`${threshold}%`} />
        <KvRow k="pipeline" v={status?.active_job?.status ?? "idle"} ok={status?.active_job?.status === "running"} />
        <KvRow k="samples" v={fmt(status?.adapter_worthy_samples)} />
      </div>
      <Divider />
      <SectionLabel>WORKFLOW</SectionLabel>
      <SidebarJump href="#sleep-control" label="Run curation" detail="1" />
      <SidebarJump href="#curated-runs" label="Review artifacts" detail="2" />
      <Link className="mt-2 block px-2 text-[10px] text-phosphor hover:underline" to="/adapters">
        Manage adapters →
      </Link>
    </>
  );
}

function AdaptersContext({ count, loaded }: { count: number | null; loaded: number | null }) {
  return (
    <>
      <SectionLabel>ADAPTATION</SectionLabel>
      <div className="grid grid-cols-2 gap-2">
        <StatCard label="ON DISK" value={count == null ? "—" : String(count)} />
        <StatCard label="LOADED" value={loaded == null ? "—" : String(loaded)} accent />
      </div>
      <div className="mt-3 rounded-[5px] border border-border-subtle bg-bg/35 px-2.5 py-2 font-mono text-[9px] leading-relaxed text-ink-muted">
        Apply a tested stack before chat. Stack order changes the final behavior.
      </div>
      <Divider />
      <SectionLabel>WORKFLOW</SectionLabel>
      <SidebarJump href="#active-stack" label="Configure stack" detail="1" />
      <SidebarJump href="#available-adapters" label="Choose adapters" detail="2" />
      <SidebarJump href="#eval-report" label="Read evaluation" detail="3" />
    </>
  );
}

function SettingsContext({ healthy, total }: { healthy: number | null; total: number | null }) {
  const links = [
    ["#appearance", "Appearance"],
    ["#hud-panels", "HUD panels"],
    ["#models-retrieval", "Models & retrieval"],
    ["#generation", "Generation"],
    ["#memory-settings", "Memory"],
    ["#document-import", "Document import"],
    ["#graph-settings", "Graph"],
    ["#sleep-settings", "Sleep"],
    ["#adaptation", "Adaptation"],
    ["#doctor", "Doctor"],
  ];
  return (
    <>
      <SectionLabel>CONFIGURATION</SectionLabel>
      <div className="rounded-[5px] border border-phosphor/30 bg-accent-soft px-2.5 py-2">
        <div className="font-mono text-[9px] text-ink-muted">HEALTH CHECKS</div>
        <div className="mt-1 text-[12px] font-semibold text-ink">
          {healthy == null || total == null ? "Loading…" : `${healthy} / ${total} passing`}
        </div>
      </div>
      <Divider />
      <SectionLabel>JUMP TO</SectionLabel>
      <div className="flex flex-col gap-0.5">
        {links.map(([href, label]) => <SidebarJump key={href} href={href} label={label} />)}
      </div>
    </>
  );
}

export function ContextSidebar() {
  const { pathname } = useLocation();
  const [params] = useSearchParams();
  const pageId = resolvePageId(pathname) ?? "chat";

  const [overview, setOverview] = useState<GraphOverview | null>(null);
  const [reviewCount, setReviewCount] = useState<number | null>(null);
  const [trashCount, setTrashCount] = useState<number | null>(null);
  const [modelInfo, setModelInfo] = useState<ModelInfo | null>(null);
  const [modelCount, setModelCount] = useState<number | null>(null);
  const [documentCount, setDocumentCount] = useState<number | null>(null);
  const [indexHealth, setIndexHealth] = useState<IndexHealth | null>(null);
  const [sleepStatus, setSleepStatus] = useState<SleepStatus | null>(null);
  const [curatedRunCount, setCuratedRunCount] = useState<number | null>(null);
  const [adapterCount, setAdapterCount] = useState<number | null>(null);
  const [loadedAdapterCount, setLoadedAdapterCount] = useState<number | null>(null);
  const [doctorHealthy, setDoctorHealthy] = useState<number | null>(null);
  const [doctorTotal, setDoctorTotal] = useState<number | null>(null);
  const [sidebarError, setSidebarError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setSidebarError(null);

    const loadContext = async () => {
      try {
        if (pageId === "graph") {
          const value = await api.graphOverview();
          if (alive) setOverview(value);
        } else if (pageId === "memory") {
          const [review, trash] = await Promise.all([
            api.listMemoryCandidates(),
            api.listMemoryTrash(),
          ]);
          if (alive) {
            setReviewCount(review.length);
            setTrashCount(trash.length);
          }
        } else if (pageId === "models") {
          const [info, models] = await Promise.all([api.getModelInfo(), api.listModels()]);
          if (alive) {
            setModelInfo(info);
            setModelCount(models.length);
          }
        } else if (pageId === "documents") {
          const [documents, health] = await Promise.all([api.listDocuments(), api.indexHealth()]);
          if (alive) {
            setDocumentCount(documents.length);
            setIndexHealth(health);
          }
        } else if (pageId === "search") {
          const health = await api.indexHealth();
          if (alive) setIndexHealth(health);
        } else if (pageId === "sleep") {
          const [status, runs] = await Promise.all([api.sleepStatus(), api.listCuratedRuns()]);
          if (alive) {
            setSleepStatus(status);
            setCuratedRunCount(runs.length);
          }
        } else if (pageId === "adapters") {
          const [adapters, loaded] = await Promise.all([api.listAdapters(), api.loadedAdapters()]);
          if (alive) {
            setAdapterCount(adapters.length);
            setLoadedAdapterCount(loaded.length);
          }
        } else if (pageId === "settings") {
          const report = await api.doctor();
          if (alive) {
            setDoctorHealthy(report.checks.filter((check) => check.ok).length);
            setDoctorTotal(report.checks.length);
          }
        }
      } catch (error) {
        if (alive) setSidebarError(String(error));
      }
    };

    void loadContext();
    const interval = window.setInterval(() => void loadContext(), 10000);

    return () => {
      alive = false;
      window.clearInterval(interval);
    };
  }, [pageId]);

  return (
    <aside className="quiet-scrollbar flex w-[196px] shrink-0 flex-col overflow-y-auto border-r border-border bg-surface/92 px-2.5 py-3.5 shadow-[10px_0_28px_rgba(0,0,0,0.16)]">
      {pageId === "graph" && <GraphContext overview={overview} />}
      {pageId === "chat" && <ChatContext />}
      {pageId === "memory" && (
        <MemoryContext reviewCount={reviewCount} trashCount={trashCount} store={params.get("store") ?? "active"} />
      )}
      {pageId === "models" && <ModelsContext info={modelInfo} count={modelCount} />}
      {pageId === "documents" && <DocumentsContext count={documentCount} health={indexHealth} />}
      {pageId === "search" && <SearchContext health={indexHealth} />}
      {pageId === "sleep" && <SleepContext status={sleepStatus} runs={curatedRunCount} />}
      {pageId === "adapters" && <AdaptersContext count={adapterCount} loaded={loadedAdapterCount} />}
      {pageId === "settings" && <SettingsContext healthy={doctorHealthy} total={doctorTotal} />}
      {sidebarError && (
        <p className="mt-auto pt-4 font-mono text-[9px] leading-relaxed text-rose">
          Context unavailable: {sidebarError}
        </p>
      )}
    </aside>
  );
}
