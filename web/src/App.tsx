import { Routes, Route, Navigate } from "react-router-dom";
import AppShell from "./components/AppShell";
import DashboardRouter from "./components/DashboardRouter";

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/dashboard/:tab" element={<DashboardRouter />} />
        <Route path="/chat" element={null} />
        <Route path="/chat/:sessionId" element={null} />
        <Route path="/" element={<Navigate to="/dashboard/overview" replace />} />
        <Route path="*" element={<Navigate to="/dashboard/overview" replace />} />
      </Route>
    </Routes>
  );
}
