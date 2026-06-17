import { BrowserRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { Layout } from "./components/Layout";
import { resolvePageId } from "./routes";
import "./App.css";

function RouteGuard({ children }: { children: React.ReactNode }) {
  const { pathname } = useLocation();
  if (resolvePageId(pathname) === null) {
    return <Navigate to="/" replace />;
  }
  return children;
}

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route
          path="*"
          element={
            <RouteGuard>
              <Layout />
            </RouteGuard>
          }
        />
      </Routes>
    </BrowserRouter>
  );
}
