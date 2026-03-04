/**
 * Zustand UI store for multi-view layout state.
 *
 * Manages ephemeral UI coordination:
 * - View mode (kanban/canvas/list)
 * - Repo and topic selection
 * - Detail panel visibility and height
 *
 * Server state (repos/topics/envs) stays in TanStack Query.
 */

import { create } from "zustand";
import type { RepoId, TopicId } from "../generated/types.js";

export type ViewMode = "kanban" | "canvas" | "list";

const STORAGE_PREFIX = "restack.ui.v1";

const PANEL_HEIGHT_KEY = `${STORAGE_PREFIX}.detailPanelHeight`;
const VIEW_MODE_KEY = `${STORAGE_PREFIX}.viewMode`;

const DEFAULT_PANEL_HEIGHT = 320;
export const PANEL_HEIGHT_MIN = 120;
export const PANEL_HEIGHT_MAX_VH = 0.6;

export function clampPanelHeight(height: number): number {
  const maxPx = Math.floor(window.innerHeight * PANEL_HEIGHT_MAX_VH);
  return Math.max(PANEL_HEIGHT_MIN, Math.min(height, maxPx));
}

function loadNumber(key: string, fallback: number): number {
  try {
    const stored = localStorage.getItem(key);
    if (stored === null) return fallback;
    const value = Number.parseInt(stored, 10);
    return Number.isNaN(value) ? fallback : value;
  } catch {
    return fallback;
  }
}

function loadString<T extends string>(key: string, fallback: T, valid: readonly T[]): T {
  try {
    const stored = localStorage.getItem(key);
    if (stored === null) return fallback;
    return valid.includes(stored as T) ? (stored as T) : fallback;
  } catch {
    return fallback;
  }
}

function save(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private browsing or quota exceeded
  }
}

const VIEW_MODES = ["kanban", "canvas", "list"] as const;

interface UIState {
  viewMode: ViewMode;
  selectedRepoId: RepoId | null;
  selectedTopicId: TopicId | null;
  detailPanelOpen: boolean;
  panelHeight: number;
}

interface UIActions {
  setViewMode: (mode: ViewMode) => void;
  setSelectedRepoId: (id: RepoId | null) => void;
  setSelectedTopicId: (id: TopicId | null) => void;
  toggleDetailPanel: () => void;
  setDetailPanelOpen: (open: boolean) => void;
  setPanelHeight: (height: number) => void;
}

export type UIStore = UIState & UIActions;

export const useUIStore = create<UIStore>((set) => ({
  viewMode: loadString(VIEW_MODE_KEY, "kanban", VIEW_MODES),
  selectedRepoId: null,
  selectedTopicId: null,
  detailPanelOpen: false,
  panelHeight: clampPanelHeight(loadNumber(PANEL_HEIGHT_KEY, DEFAULT_PANEL_HEIGHT)),

  setViewMode: (mode) => {
    save(VIEW_MODE_KEY, mode);
    set({ viewMode: mode });
  },

  setSelectedRepoId: (id) => set({ selectedRepoId: id }),

  setSelectedTopicId: (id) =>
    set((state) => ({
      selectedTopicId: id,
      detailPanelOpen: id !== null ? true : state.detailPanelOpen,
    })),

  toggleDetailPanel: () =>
    set((state) => ({ detailPanelOpen: !state.detailPanelOpen })),

  setDetailPanelOpen: (open) => set({ detailPanelOpen: open }),

  setPanelHeight: (height) => {
    const clamped = clampPanelHeight(height);
    save(PANEL_HEIGHT_KEY, String(clamped));
    set({ panelHeight: clamped });
  },
}));
