import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function useTauriEvent<T>(
  event: string,
  handler: (payload: T) => void,
  deps: unknown[] = [],
) {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<T>(event, (e) => {
      if (!cancelled) handler(e.payload);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
