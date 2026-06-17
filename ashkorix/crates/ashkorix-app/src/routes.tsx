import type { ComponentType } from "react";
import { ChatPage } from "./pages/ChatPage";
import { ModelsPage } from "./pages/ModelsPage";
import { DocumentsPage } from "./pages/DocumentsPage";
import { SearchPage } from "./pages/SearchPage";
import { MemoryPage } from "./pages/MemoryPage";
import { SettingsPage } from "./pages/SettingsPage";
import type { PageId } from "./types";

export interface AppPage {
  id: PageId;
  label: string;
  path: string;
  Component: ComponentType;
}

export const APP_PAGES: AppPage[] = [
  { id: "chat", label: "Chat", path: "/", Component: ChatPage },
  { id: "models", label: "Models", path: "/models", Component: ModelsPage },
  { id: "documents", label: "Documents", path: "/documents", Component: DocumentsPage },
  { id: "search", label: "Search", path: "/search", Component: SearchPage },
  { id: "memory", label: "Memory", path: "/memory", Component: MemoryPage },
  { id: "settings", label: "Settings", path: "/settings", Component: SettingsPage },
];

const PAGE_BY_PATH = new Map(APP_PAGES.map((page) => [page.path, page.id]));

export function resolvePageId(pathname: string): PageId | null {
  if (pathname === "/" || pathname === "") return "chat";
  const normalized = pathname.endsWith("/") && pathname.length > 1
    ? pathname.slice(0, -1)
    : pathname;
  return PAGE_BY_PATH.get(normalized) ?? null;
}
