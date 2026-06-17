import { NavLink } from "react-router-dom";
import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { APP_PAGES } from "../routes";
import { KeepAlivePages } from "./KeepAlivePages";

export function Layout() {
  const [version, setVersion] = useState("…");
  const [inboxCount, setInboxCount] = useState(0);
  const [cudaAvailable, setCudaAvailable] = useState(false);

  const loadInboxCount = useCallback(() => {
    api.listMemoryCandidates().then((c) => setInboxCount(c.length)).catch(() => {});
  }, []);

  useEffect(() => {
    api.getVersion().then(setVersion).catch(() => setVersion("unknown"));
    api.getCudaStatus().then((s) => setCudaAvailable(s.available)).catch(() => setCudaAvailable(false));
    loadInboxCount();
    const interval = setInterval(loadInboxCount, 30000);
    return () => clearInterval(interval);
  }, [loadInboxCount]);

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand-inline">
          <h1>Ashkorix</h1>
          <span>v{version} · local only</span>
          <span
            className={`cuda-status${cudaAvailable ? " cuda-on" : " cuda-off"}`}
            title={
              cudaAvailable
                ? "CUDA GPU detected — models offload to GPU automatically"
                : "No CUDA GPU — inference uses CPU"
            }
          >
            {cudaAvailable ? "CUDA ON" : "CUDA OFF"}
          </span>
        </div>
        <nav className="header-nav">
          {APP_PAGES.map((item) => (
            <NavLink
              key={item.id}
              to={item.path}
              end={item.path === "/"}
              className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
            >
              {item.label}
              {item.id === "memory" && inboxCount > 0 ? ` (${inboxCount})` : ""}
            </NavLink>
          ))}
        </nav>
        <button type="button" className="btn" onClick={() => api.openDataFolder()}>
          Open Data folder
        </button>
      </header>
      <main className="page-content">
        <KeepAlivePages />
      </main>
    </div>
  );
}
