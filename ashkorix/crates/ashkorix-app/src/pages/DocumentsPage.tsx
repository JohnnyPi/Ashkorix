import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { Document, ImporterInfo, IndexHealth } from "../types";

export function DocumentsPage() {
  const [documents, setDocuments] = useState<Document[]>([]);
  const [importers, setImporters] = useState<ImporterInfo[]>([]);
  const [health, setHealth] = useState<IndexHealth | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    const [docs, h] = await Promise.all([api.listDocuments(), api.indexHealth()]);
    setDocuments(docs);
    setHealth(h);
  };

  useEffect(() => {
    Promise.all([refresh(), api.listImporters().then(setImporters)]).catch((e) =>
      setError(String(e)),
    );
  }, []);

  const importFiles = async () => {
    setError(null);
    setMessage(null);
    const selected = await open({
      multiple: true,
      directory: false,
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    setBusy(true);
    try {
      const results = await api.importFiles(paths);
      const ok = results.filter((r) => r.status !== "Failed").length;
      setMessage(`Imported ${ok} of ${results.length} file(s). Rebuild the index to search.`);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const buildIndex = async (rebuild: boolean) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const h = rebuild ? await api.rebuildIndex() : await api.buildIndex();
      setHealth(h);
      setMessage(rebuild ? "Index rebuilt." : "Index built.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const removeDoc = async (id: string) => {
    if (!confirm("Delete this document and its chunks?")) return;
    setBusy(true);
    setError(null);
    try {
      await api.deleteDocument(id);
      await refresh();
      setMessage("Document deleted. Rebuild the index to update search.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="panel">
        <h3>Knowledge base</h3>
        <p className="muted" style={{ marginTop: 0 }}>
          Import files into the shared document pool. All documents feed one global index under{" "}
          <code>Data/documents/</code> and <code>Data/index/</code>.
        </p>

        {health && (
          <p className="muted">
            {health.indexed ? (
              <>
                <span className="badge badge-ok">indexed</span> {health.chunk_count} chunks ·{" "}
                {health.vector_count} vectors · {health.lexical_count} lexical docs
              </>
            ) : (
              <>
                <span className="badge badge-warn">not indexed</span> — {health.message}
              </>
            )}
            {!health.embedding_loaded && (
              <span style={{ display: "block", marginTop: "0.35rem" }}>
                Embedding model not loaded. Open Settings, pick a <code>.gguf</code> embedding file,
                save config, then rebuild.
              </span>
            )}
          </p>
        )}

        <div className="row" style={{ marginTop: "0.75rem" }}>
          <button type="button" className="btn btn-primary" onClick={importFiles} disabled={busy}>
            Import files…
          </button>
          <button type="button" className="btn" onClick={() => buildIndex(false)} disabled={busy}>
            Build index
          </button>
          <button type="button" className="btn" onClick={() => buildIndex(true)} disabled={busy}>
            Rebuild index
          </button>
        </div>

        {importers.length > 0 && (
          <details style={{ marginTop: "0.75rem" }}>
            <summary className="muted">Supported file types</summary>
            <ul className="muted" style={{ margin: "0.5rem 0 0", paddingLeft: "1.25rem" }}>
              {importers.map((i) => (
                <li key={i.id}>
                  {i.name}: {i.extensions.join(", ")}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>

      <div className="panel">
        <h3>Documents ({documents.length})</h3>
        {documents.length === 0 ? (
          <p className="muted">No documents yet. Import files to get started.</p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Filename</th>
                <th>Type</th>
                <th>Chunks</th>
                <th>Status</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {documents.map((doc) => (
                <tr key={doc.id}>
                  <td>{doc.original_filename}</td>
                  <td>{doc.file_type}</td>
                  <td>{doc.chunk_count}</td>
                  <td>{doc.import_status}</td>
                  <td>
                    <button
                      type="button"
                      className="btn btn-danger"
                      onClick={() => removeDoc(doc.id)}
                      disabled={busy}
                    >
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {message && <p className="success">{message}</p>}
      {error && <p className="error">{error}</p>}
    </>
  );
}
