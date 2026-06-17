import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type {
  CreateMemoryInput,
  EditCandidateInput,
  Memory,
  MemoryCandidate,
  MemoryType,
} from "../types";

const MEMORY_TYPES: MemoryType[] = [
  "user_preference",
  "project_fact",
  "decision",
  "procedure",
];

const TYPE_LABELS: Record<MemoryType, string> = {
  user_preference: "User Preference",
  project_fact: "Project Fact",
  decision: "Decision",
  procedure: "Procedure",
};

type Tab = "active" | "inbox";

export function MemoryPage() {
  const [tab, setTab] = useState<Tab>("active");
  const [memories, setMemories] = useState<Memory[]>([]);
  const [candidates, setCandidates] = useState<MemoryCandidate[]>([]);
  const [scopeFilter, setScopeFilter] = useState("");
  const [search, setSearch] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [editingCandidate, setEditingCandidate] = useState<MemoryCandidate | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newMemory, setNewMemory] = useState<CreateMemoryInput>({
    memory_type: "project_fact",
    scope: "project:ashkorix",
    title: "",
    content: "",
    importance: 0.75,
    confidence: 1.0,
  });

  const load = useCallback(async () => {
    setError(null);
    try {
      const [mems, inbox] = await Promise.all([
        search.trim()
          ? api.searchMemories(search.trim())
          : api.listMemories(scopeFilter.trim() || undefined),
        api.listMemoryCandidates(),
      ]);
      setMemories(mems);
      setCandidates(inbox);
    } catch (e) {
      setError(String(e));
    }
  }, [scopeFilter, search]);

  useEffect(() => {
    load();
  }, [load]);

  const approve = async (id: string) => {
    try {
      await api.approveMemoryCandidate(id);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const reject = async (id: string) => {
    try {
      await api.rejectMemoryCandidate(id);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const editApprove = async (id: string, edit: EditCandidateInput) => {
    try {
      await api.editAndApproveCandidate(id, edit);
      setEditingCandidate(null);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const deactivate = async (id: string) => {
    try {
      await api.deactivateMemory(id);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const create = async () => {
    try {
      await api.createMemory(newMemory);
      setShowCreate(false);
      setNewMemory({
        memory_type: "project_fact",
        scope: "project:ashkorix",
        title: "",
        content: "",
        importance: 0.75,
        confidence: 1.0,
      });
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <>
      <div className="panel">
        <div className="row" style={{ marginBottom: "0.75rem" }}>
          <h2 style={{ margin: 0 }}>Memory</h2>
          <div className="row">
            <button
              type="button"
              className={`btn${tab === "active" ? " primary" : ""}`}
              onClick={() => setTab("active")}
            >
              Active ({memories.length})
            </button>
            <button
              type="button"
              className={`btn${tab === "inbox" ? " primary" : ""}`}
              onClick={() => setTab("inbox")}
            >
              Inbox ({candidates.length})
            </button>
            {tab === "active" && (
              <button type="button" className="btn" onClick={() => setShowCreate((v) => !v)}>
                {showCreate ? "Cancel" : "Add memory"}
              </button>
            )}
          </div>
        </div>

        {error && <p className="error">{error}</p>}

        {tab === "active" && (
          <>
            <div className="row" style={{ marginBottom: "0.75rem" }}>
              <input
                className="input"
                placeholder="Filter by scope (e.g. project:ashkorix)"
                value={scopeFilter}
                onChange={(e) => setScopeFilter(e.target.value)}
              />
              <input
                className="input"
                placeholder="Search title or content"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
              <button type="button" className="btn" onClick={load}>
                Refresh
              </button>
            </div>

            {showCreate && (
              <div className="panel nested" style={{ marginBottom: "1rem" }}>
                <h3>New memory</h3>
                <div className="form-grid">
                  <label>
                    Type
                    <select
                      value={newMemory.memory_type}
                      onChange={(e) =>
                        setNewMemory({ ...newMemory, memory_type: e.target.value as MemoryType })
                      }
                    >
                      {MEMORY_TYPES.map((t) => (
                        <option key={t} value={t}>
                          {TYPE_LABELS[t]}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Scope
                    <input
                      className="input"
                      value={newMemory.scope}
                      onChange={(e) => setNewMemory({ ...newMemory, scope: e.target.value })}
                    />
                  </label>
                  <label>
                    Title
                    <input
                      className="input"
                      value={newMemory.title}
                      onChange={(e) => setNewMemory({ ...newMemory, title: e.target.value })}
                    />
                  </label>
                  <label>
                    Content
                    <textarea
                      className="input"
                      rows={3}
                      value={newMemory.content}
                      onChange={(e) => setNewMemory({ ...newMemory, content: e.target.value })}
                    />
                  </label>
                </div>
                <button type="button" className="btn primary" onClick={create}>
                  Save
                </button>
              </div>
            )}

            {memories.length === 0 ? (
              <p className="muted">No active memories.</p>
            ) : (
              <ul className="list">
                {memories.map((m) => (
                  <li key={m.id} className="list-item">
                    <div className="row">
                      <strong>{m.title}</strong>
                      <span className="muted">{TYPE_LABELS[m.memory_type]}</span>
                    </div>
                    <p>{m.content}</p>
                    <div className="row muted small">
                      <span>{m.scope}</span>
                      <span>importance {m.importance.toFixed(2)}</span>
                      <span>confidence {m.confidence.toFixed(2)}</span>
                    </div>
                    <button type="button" className="btn" onClick={() => deactivate(m.id)}>
                      Deactivate
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}

        {tab === "inbox" && (
          <>
            {candidates.length === 0 ? (
              <p className="muted">No pending candidates. Use Extract memories on the Chat page.</p>
            ) : (
              <ul className="list">
                {candidates.map((c) => (
                  <li key={c.id} className="list-item">
                    {editingCandidate?.id === c.id ? (
                      <CandidateEditor
                        candidate={c}
                        onCancel={() => setEditingCandidate(null)}
                        onSave={(edit) => editApprove(c.id, edit)}
                      />
                    ) : (
                      <>
                        <div className="row">
                          <strong>{c.proposed_title}</strong>
                          <span className="muted">{TYPE_LABELS[c.proposed_type]}</span>
                        </div>
                        <p>{c.proposed_content}</p>
                        {c.reason && <p className="muted small">Reason: {c.reason}</p>}
                        <div className="row muted small">
                          <span>{c.proposed_scope}</span>
                          <span>confidence {c.confidence.toFixed(2)}</span>
                        </div>
                        <div className="row">
                          <button type="button" className="btn primary" onClick={() => approve(c.id)}>
                            Approve
                          </button>
                          <button type="button" className="btn" onClick={() => setEditingCandidate(c)}>
                            Edit
                          </button>
                          <button type="button" className="btn" onClick={() => reject(c.id)}>
                            Reject
                          </button>
                        </div>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </div>
    </>
  );
}

function CandidateEditor({
  candidate,
  onCancel,
  onSave,
}: {
  candidate: MemoryCandidate;
  onCancel: () => void;
  onSave: (edit: EditCandidateInput) => void;
}) {
  const [edit, setEdit] = useState<EditCandidateInput>({
    proposed_type: candidate.proposed_type,
    proposed_scope: candidate.proposed_scope,
    proposed_title: candidate.proposed_title,
    proposed_content: candidate.proposed_content,
    importance: candidate.importance,
    confidence: candidate.confidence,
  });

  return (
    <div className="form-grid">
      <label>
        Type
        <select
          value={edit.proposed_type}
          onChange={(e) =>
            setEdit({ ...edit, proposed_type: e.target.value as MemoryType })
          }
        >
          {MEMORY_TYPES.map((t) => (
            <option key={t} value={t}>
              {TYPE_LABELS[t]}
            </option>
          ))}
        </select>
      </label>
      <label>
        Scope
        <input
          className="input"
          value={edit.proposed_scope}
          onChange={(e) => setEdit({ ...edit, proposed_scope: e.target.value })}
        />
      </label>
      <label>
        Title
        <input
          className="input"
          value={edit.proposed_title}
          onChange={(e) => setEdit({ ...edit, proposed_title: e.target.value })}
        />
      </label>
      <label>
        Content
        <textarea
          className="input"
          rows={3}
          value={edit.proposed_content}
          onChange={(e) => setEdit({ ...edit, proposed_content: e.target.value })}
        />
      </label>
      <div className="row">
        <button type="button" className="btn primary" onClick={() => onSave(edit)}>
          Save &amp; approve
        </button>
        <button type="button" className="btn" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
