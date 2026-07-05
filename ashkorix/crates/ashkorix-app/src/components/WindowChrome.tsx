import { cn } from "../lib/cn";

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function controlWindow(action: "minimize" | "maximize" | "close") {
  if (!isTauri) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    if (action === "minimize") await w.minimize();
    else if (action === "maximize") await w.toggleMaximize();
    else await w.close();
  } catch {
    /* not running inside a Tauri window — controls are a no-op */
  }
}

function ControlButton({
  label,
  glyph,
  danger,
  onClick,
}: {
  label: string;
  glyph: string;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "flex h-6 w-7 items-center justify-center rounded text-[13px] leading-none text-ink-muted transition-colors",
        danger ? "hover:bg-error/20 hover:text-error" : "hover:bg-white/5 hover:text-ink",
      )}
    >
      {glyph}
    </button>
  );
}

export function WindowChrome() {
  return (
    <div
      data-tauri-drag-region
      className="flex h-8 shrink-0 select-none items-center justify-between border-b border-border-subtle bg-surface px-3"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5">
        <span className="h-[11px] w-[11px] shrink-0 rounded-[2px] bg-rose shadow-[0_0_8px_rgba(239,106,82,0.5)]" />
        <span className="font-mono text-xs tracking-wide text-ink-muted">Ashkorix</span>
      </div>
      <div className="flex items-center gap-0.5">
        <ControlButton label="Minimize" glyph="─" onClick={() => controlWindow("minimize")} />
        <ControlButton label="Maximize" glyph="▢" onClick={() => controlWindow("maximize")} />
        <ControlButton label="Close" glyph="✕" danger onClick={() => controlWindow("close")} />
      </div>
    </div>
  );
}