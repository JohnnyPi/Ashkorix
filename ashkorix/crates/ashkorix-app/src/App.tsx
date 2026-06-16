import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Layout } from "./components/Layout";
import { ChatPage } from "./pages/ChatPage";
import { ModelsPage } from "./pages/ModelsPage";
import { DocumentsPage } from "./pages/DocumentsPage";
import { SearchPage } from "./pages/SearchPage";
import { SettingsPage } from "./pages/SettingsPage";
import "./App.css";

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<ChatPage />} />
          <Route path="models" element={<ModelsPage />} />
          <Route path="documents" element={<DocumentsPage />} />
          <Route path="search" element={<SearchPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
