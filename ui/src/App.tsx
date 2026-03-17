/**
 * Main application layout:
 * - Header: Logo, view tabs (Kanban/Canvas/List), repo selector
 * - Main: Active view
 * - Bottom: Collapsible detail panel
 */

import { useEffect, lazy, Suspense } from "react";
import { useUIStore, type ViewMode } from "./lib/store.js";
import { useWebSocketSync } from "./lib/websocket.js";
import { Header } from "./components/Header.js";
import { DetailPanel } from "./components/DetailPanel.js";
import { ErrorBoundary } from "./components/ErrorBoundary.js";
import { KanbanView } from "./components/views/kanban/KanbanView.js";
import { ListView } from "./components/views/ListView.js";
import { Toaster } from "sonner";

/** Lazy-loaded: ReactFlow + dagre only downloaded when Canvas view is opened. */
const CanvasView = lazy(() =>
  import("./components/views/CanvasView.js").then((m) => ({ default: m.CanvasView })),
);

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

      <main className="flex-1 flex flex-col min-h-0" aria-label={`${viewMode} view`}>
        <div className="flex-1 min-h-0 flex">
          <ErrorBoundary>
            <ViewContainer viewMode={viewMode} />
          </ErrorBoundary>
        </div>
        <ErrorBoundary>
          <DetailPanel />
        </ErrorBoundary>
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
      return (
        <Suspense fallback={<ViewLoading />}>
          <CanvasView />
        </Suspense>
      );
    case "list":
      return <ListView />;
  }
}

function ViewLoading() {
  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="w-16 h-1 rounded-full bg-border animate-skeleton-pulse" />
    </div>
  );
}
