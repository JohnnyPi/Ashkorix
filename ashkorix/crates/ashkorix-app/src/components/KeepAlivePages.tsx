import { useLocation } from "react-router-dom";
import { APP_PAGES, resolvePageId } from "../routes";

export function KeepAlivePages() {
  const { pathname } = useLocation();
  const activeId = resolvePageId(pathname) ?? "chat";

  return (
    <>
      {APP_PAGES.map(({ id, Component }) => (
        <div
          key={id}
          className={`page-panel${id === activeId ? " active" : ""}`}
          aria-hidden={id !== activeId}
        >
          <Component />
        </div>
      ))}
    </>
  );
}
