import { useEffect, useState } from "react";
import { api, formatBytes } from "../api";
import type { ModelFileInfo, ModelInfo } from "../types";

export function ModelsPage() {
  const [models, setModels] = useState<ModelFileInfo[]>([]);
  const [loaded, setLoaded] = useState<ModelInfo | null>(null);
  const [loading, setLoading] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const [list, info] = await Promise.all([api.listModels(), api.getModelInfo()]);
      setModels(list);
      setLoaded(info);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const load = async (path: string) => {
    setLoading(path);
    setError(null);
    try {
      const config = await api.getConfig();
      await api.loadModel(path, {
        n_ctx: config.generation.context_size,
        n_gpu_layers: config.generation.gpu_layers,
        threads: config.generation.threads,
      });
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(null);
    }
  };

  const unload = async () => {
    setError(null);
    try {
      await api.unloadModel();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <>
      <div className="panel">
        <h3>Loaded model</h3>
        {loaded ? (
          <div className="row">
            <span className="badge badge-ok">{loaded.filename}</span>
            <span className="muted">ctx {loaded.n_ctx}</span>
            {loaded.architecture && <span className="muted">{loaded.architecture}</span>}
            <button type="button" className="btn btn-danger" onClick={unload}>
              Unload
            </button>
          </div>
        ) : (
          <p className="muted">No model loaded. Place `.gguf` files in `Data/models/` or open the Data folder.</p>
        )}
      </div>

      <div className="panel">
        <div className="row" style={{ justifyContent: "space-between", marginBottom: "0.75rem" }}>
          <h3 style={{ margin: 0 }}>Discovered GGUF models</h3>
          <button type="button" className="btn" onClick={refresh}>
            Refresh
          </button>
        </div>
        {models.length === 0 ? (
          <p className="muted">No `.gguf` files found under `models_dir`.</p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>File</th>
                <th>Size</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {models.map((m) => (
                <tr key={m.path}>
                  <td>
                    <code>{m.filename}</code>
                  </td>
                  <td>{formatBytes(m.size_bytes)}</td>
                  <td>
                    <button
                      type="button"
                      className="btn btn-primary"
                      disabled={loading === m.path}
                      onClick={() => load(m.path)}
                    >
                      {loading === m.path ? "Loading…" : "Load"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {error && <p className="error">{error}</p>}
      </div>
    </>
  );
}
