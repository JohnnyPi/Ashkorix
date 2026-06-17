import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { AshkorixConfig, DoctorReport, GenerationConfig, ModelFileInfo } from "../types";

function ggufModels(models: ModelFileInfo[], filter: (name: string) => boolean) {
  return models.filter((m) => filter(m.filename.toLowerCase()));
}

export function SettingsPage() {
  const [config, setConfig] = useState<AshkorixConfig | null>(null);
  const [gen, setGen] = useState<GenerationConfig | null>(null);
  const [doctor, setDoctor] = useState<DoctorReport | null>(null);
  const [models, setModels] = useState<ModelFileInfo[]>([]);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      api.getConfig(),
      api.getGenerationSettings(),
      api.doctor(),
      api.listModels(),
    ])
      .then(([c, g, d, m]) => {
        setConfig(c);
        setGen(g);
        setDoctor(d);
        setModels(m);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const saveConfig = async () => {
    if (!config) return;
    setError(null);
    setSaved(false);
    try {
      await api.updateConfig(config);
      const [c, d] = await Promise.all([api.getConfig(), api.doctor()]);
      setConfig(c);
      setDoctor(d);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const saveGen = async () => {
    if (!gen) return;
    setError(null);
    setSaved(false);
    try {
      await api.setGenerationSettings(gen);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const runDoctor = async () => {
    setError(null);
    try {
      setDoctor(await api.doctor());
    } catch (e) {
      setError(String(e));
    }
  };

  const browseGguf = async (field: "embedding_model_path" | "reranker_model_path") => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "GGUF model", extensions: ["gguf"] }],
    });
    if (!selected || typeof selected !== "string") return;
    setConfig((c) => (c ? { ...c, [field]: selected } : c));
  };

  if (!config || !gen) {
    return <p className="muted">Loading settings…</p>;
  }

  const embeddingCandidates = ggufModels(
    models,
    (n) => n.includes("embed") || n.includes("nomic") || n.includes("bge") || n.includes("e5"),
  );
  const rerankerCandidates = ggufModels(
    models,
    (n) => n.includes("rerank") || n.includes("cross") || n.includes("jina"),
  );

  return (
    <>
      <div className="panel">
        <h3>Data paths</h3>
        <p className="muted">data_dir</p>
        <code style={{ display: "block", marginBottom: "0.75rem", wordBreak: "break-all" }}>
          {config.data_dir}
        </code>
        <p className="muted">models_dir</p>
        <code style={{ display: "block", marginBottom: "0.75rem", wordBreak: "break-all" }}>
          {config.models_dir}
        </code>
        <button type="button" className="btn" onClick={() => api.openDataFolder()}>
          Open Data folder
        </button>
      </div>

      <div className="panel">
        <h3>Models & retrieval</h3>
        <p className="muted" style={{ marginTop: 0 }}>
          Use a <strong>.gguf file path</strong>, not a folder. You can browse or pick from discovered
          models below.
        </p>

        <label className="muted" style={{ display: "block", marginBottom: "0.5rem" }}>
          Embedding model path
          <div className="row" style={{ marginTop: "0.25rem", gap: "0.5rem" }}>
            <input
              className="input"
              style={{ flex: 1 }}
              value={config.embedding_model_path ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  embedding_model_path: e.target.value || null,
                })
              }
              placeholder="E:\...\nomic-embed-text-v1.5.Q8_0.gguf"
            />
            <button type="button" className="btn" onClick={() => browseGguf("embedding_model_path")}>
              Browse…
            </button>
          </div>
        </label>
        {embeddingCandidates.length > 0 && (
          <label className="muted" style={{ display: "block", marginBottom: "0.5rem" }}>
            Or select embedding model
            <select
              className="select"
              style={{ display: "block", width: "100%", marginTop: "0.25rem" }}
              value={config.embedding_model_path ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  embedding_model_path: e.target.value || null,
                })
              }
            >
              <option value="">—</option>
              {embeddingCandidates.map((m) => (
                <option key={m.path} value={m.path}>
                  {m.filename}
                </option>
              ))}
            </select>
          </label>
        )}

        <label className="muted" style={{ display: "block", marginBottom: "0.5rem" }}>
          Reranker model path (optional)
          <div className="row" style={{ marginTop: "0.25rem", gap: "0.5rem" }}>
            <input
              className="input"
              style={{ flex: 1 }}
              value={config.reranker_model_path ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  reranker_model_path: e.target.value || null,
                })
              }
              placeholder="Path to cross-encoder .gguf (heuristic rerank used if empty)"
            />
            <button type="button" className="btn" onClick={() => browseGguf("reranker_model_path")}>
              Browse…
            </button>
          </div>
        </label>
        {rerankerCandidates.length > 0 && (
          <label className="muted" style={{ display: "block", marginBottom: "0.5rem" }}>
            Or select reranker model
            <select
              className="select"
              style={{ display: "block", width: "100%", marginTop: "0.25rem" }}
              value={config.reranker_model_path ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  reranker_model_path: e.target.value || null,
                })
              }
            >
              <option value="">— (heuristic rerank)</option>
              {rerankerCandidates.map((m) => (
                <option key={m.path} value={m.path}>
                  {m.filename}
                </option>
              ))}
            </select>
          </label>
        )}

        <label className="muted" style={{ display: "block", marginBottom: "0.5rem" }}>
          Default retrieval mode
          <select
            className="select"
            style={{ display: "block", marginTop: "0.25rem" }}
            value={config.default_retrieval_mode}
            onChange={(e) => setConfig({ ...config, default_retrieval_mode: e.target.value })}
          >
            <option value="fast">fast</option>
            <option value="balanced">balanced</option>
            <option value="thorough">thorough</option>
            <option value="deep">deep</option>
            <option value="corpus-map">corpus-map</option>
          </select>
        </label>
        <label className="muted" style={{ display: "block", marginBottom: "0.75rem" }}>
          Log level
          <input
            className="input"
            style={{ display: "block", width: "100%", marginTop: "0.25rem" }}
            value={config.log_level}
            onChange={(e) => setConfig({ ...config, log_level: e.target.value })}
          />
        </label>
        <button type="button" className="btn btn-primary" onClick={saveConfig}>
          Save config
        </button>
        <p className="muted" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
          Saving reloads the embedding model (and reranker if set). Then rebuild the index on the
          Documents page.
        </p>
      </div>

      <div className="panel">
        <h3>Generation</h3>
        <div className="grid-2">
          {(
            [
              ["temperature", 0, 2, 0.05],
              ["top_p", 0, 1, 0.05],
              ["repeat_penalty", 0.5, 2, 0.05],
              ["max_tokens", 64, 8192, 64],
              ["context_size", 512, 32768, 512],
              ["gpu_layers", 0, 99, 1],
              ["threads", 1, 32, 1],
            ] as const
          ).map(([key, min, max, step]) => (
            <label key={key} className="muted">
              {key}{" "}
              <input
                type="number"
                className="input"
                style={{ width: "100%" }}
                min={min}
                max={max}
                step={step}
                value={gen[key]}
                onChange={(e) => setGen({ ...gen, [key]: Number(e.target.value) })}
              />
            </label>
          ))}
        </div>
        <p className="muted" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
          `gpu_layers = 0` auto-detects CUDA and offloads all layers when a compatible GPU is found;
          set a positive value to override.
        </p>
        <div className="row" style={{ marginTop: "0.75rem" }}>
          <button type="button" className="btn btn-primary" onClick={saveGen}>
            Save generation settings
          </button>
        </div>
      </div>

      <div className="panel">
        <h3>Memory</h3>
        <div className="form-grid">
          <label className="muted">
            <input
              type="checkbox"
              checked={config.memory.enabled}
              onChange={(e) =>
                setConfig({
                  ...config,
                  memory: { ...config.memory, enabled: e.target.checked },
                })
              }
            />{" "}
            Enable memory retrieval
          </label>
          <label className="muted">
            Active project
            <input
              className="input"
              list="project-presets"
              value={config.memory.active_project}
              onChange={(e) =>
                setConfig({
                  ...config,
                  memory: { ...config.memory, active_project: e.target.value },
                })
              }
            />
            <datalist id="project-presets">
              <option value="ashkorix" />
              <option value="khoraxis" />
              <option value="morrowind-tool" />
              <option value="horror-novel" />
            </datalist>
          </label>
          <label className="muted">
            Max injected memories
            <input
              type="number"
              className="input"
              min={3}
              max={8}
              value={config.memory.max_injected}
              onChange={(e) =>
                setConfig({
                  ...config,
                  memory: { ...config.memory, max_injected: Number(e.target.value) },
                })
              }
            />
          </label>
          <label className="muted">
            Min confidence
            <input
              type="number"
              className="input"
              min={0}
              max={1}
              step={0.05}
              value={config.memory.min_confidence}
              onChange={(e) =>
                setConfig({
                  ...config,
                  memory: { ...config.memory, min_confidence: Number(e.target.value) },
                })
              }
            />
          </label>
        </div>
        <p className="muted">
          Scope: global, project:{config.memory.active_project}, and current conversation.
        </p>
      </div>

      <div className="panel">
        <div className="row" style={{ justifyContent: "space-between" }}>
          <h3 style={{ margin: 0 }}>Doctor</h3>
          <button type="button" className="btn" onClick={runDoctor}>
            Re-run
          </button>
        </div>
        {doctor && (
          <>
            <p className="muted">local_only: {String(doctor.local_only)}</p>
            <table className="table">
              <thead>
                <tr>
                  <th>Check</th>
                  <th>Path</th>
                  <th>Status</th>
                  <th>Message</th>
                </tr>
              </thead>
              <tbody>
                {doctor.checks.map((c) => (
                  <tr key={c.name}>
                    <td>{c.name}</td>
                    <td style={{ wordBreak: "break-all", fontSize: "0.8rem" }}>{c.path}</td>
                    <td>
                      <span className={`badge${c.ok ? " badge-ok" : " badge-warn"}`}>
                        {c.ok ? "OK" : "FAIL"}
                      </span>
                    </td>
                    <td className="muted">{c.message}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </div>

      {saved && <p className="success">Settings saved.</p>}
      {error && <p className="error">{error}</p>}
    </>
  );
}
