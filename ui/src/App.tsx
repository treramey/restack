/**
 * Main application layout:
 * - Header: Logo, view tabs (Kanban/Canvas/List), repo selector
 * - Main: Active view
 * - Bottom: Collapsible detail panel
 */

import { useEffect } from "react";
import { useUIStore, type ViewMode } from "./lib/store.js";
import { useWebSocketSync } from "./lib/websocket.js";
import { Header } from "./components/Header.js";
import { DetailPanel } from "./components/DetailPanel.js";
import { KanbanView } from "./components/views/KanbanView.js";
import { CanvasView, ListView } from "./components/views/index.js";
import { Toaster } from "sonner";

export function App() {
  const viewMode = useUIStore((s) => s.viewMode);
  const setViewMode = useUIStore((s) => s.setViewMode);
  const toggleDetailPanel = useUIStore((s) => s.toggleDetailPanel);
  useWebSocketSync();

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      switch (e.key) {
        case "1":
          setViewMode("kanban");
          break;
        case "2":
          setViewMode("canvas");
          break;
        case "3":
          setViewMode("list");
          break;
        case "D":
        case "d":
          toggleDetailPanel();
          break;
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [setViewMode, toggleDetailPanel]);

  return (
    <div className="flex flex-col h-screen bg-bg-primary text-text-primary">
      <Header />

      <main className="flex-1 flex flex-col min-h-0">
        <div className="flex-1 min-h-0 flex">
          <ViewContainer viewMode={viewMode} />
        </div>
        <DetailPanel />
      </main>

      <Toaster position="bottom-right" theme="dark" />
    </div>
  );
}

function ViewContainer({ viewMode }: { viewMode: ViewMode }) {
  switch (viewMode) {
    case "kanban":
      return <KanbanView />;
    case "canvas":
      return <CanvasView />;
    case "list":
      return <ListView />;
  }
}
