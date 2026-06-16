import { NavLink, Outlet } from "react-router-dom";
import { useEffect, useState } from "react";
import { api } from "../api";
import type { PageId } from "../types";

const NAV: { id: PageId; label: string; path: string }[] = [
  { id: "chat", label: "Chat", path: "/" },
  { id: "models", label: "Models", path: "/models" },
  { id: "documents", label: "Documents", path: "/documents" },
  { id: "search", label: "Search", path: "/search" },
  { id: "settings", label: "Settings", path: "/settings" },
];

export function Layout() {
  const [version, setVersion] = useState("…");

  useEffect(() => {
    api.getVersion().then(setVersion).catch(() => setVersion("unknown"));
  }, []);

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand-inline">
          <h1>Ashkorix</h1>
          <span>v{version} · local only</span>
        </div>
        <nav className="header-nav">
          {NAV.map((item) => (
            <NavLink
              key={item.id}
              to={item.path}
              end={item.path === "/"}
              className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
        <button type="button" className="btn" onClick={() => api.openDataFolder()}>
          Open Data folder
        </button>
      </header>
      <main className="page-content">
        <Outlet />
      </main>
    </div>
  );
}
