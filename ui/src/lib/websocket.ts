import { useEffect, useRef, useSyncExternalStore } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "./queries.js";

type WsEvent =
  | { type: "invalidate"; queryKeys: string[][] }
  | { type: "refreshStatus"; status: "running" | "done" | "error"; error?: string };

/** Lightweight external store for refresh status — avoids re-rendering the entire query tree. */
type RefreshStatus = "idle" | "running";
let currentRefreshStatus: RefreshStatus = "idle";
const refreshListeners = new Set<() => void>();

function setRefreshStatus(status: RefreshStatus) {
  if (currentRefreshStatus === status) return;
  currentRefreshStatus = status;
  for (const listener of refreshListeners) listener();
}

export function useRefreshStatus(): RefreshStatus {
  return useSyncExternalStore(
    (cb) => { refreshListeners.add(cb); return () => { refreshListeners.delete(cb); }; },
    () => currentRefreshStatus,
  );
}

const WS_RECONNECT_BASE_MS = 3_000;
const WS_RECONNECT_MAX_MS = 60_000;
const WS_MAX_RETRIES = 20;

export function useWebSocketSync(url?: string) {
  const resolvedUrl = url ?? `${globalThis.location.protocol === "https:" ? "wss" : "ws"}://${globalThis.location.host}/ws`;
  const queryClient = useQueryClient();
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const retriesRef = useRef(0);

  useEffect(() => {
    const connect = () => {
      const ws = new WebSocket(resolvedUrl);

      ws.onopen = () => {
        wsRef.current = ws;
        retriesRef.current = 0;
      };

      ws.onmessage = (event) => {
        if (typeof event.data !== "string") return;
        let data: WsEvent;
        try { data = JSON.parse(event.data); } catch { return; }

        if (data.type === "invalidate" && Array.isArray(data.queryKeys)) {
          for (const key of data.queryKeys) {
            const rootKey = key[0];
            if (rootKey && rootKey in queryKeys) {
              const typed = rootKey as keyof typeof queryKeys;
              queryClient.invalidateQueries({
                queryKey: queryKeys[typed].all,
              });
            }
          }
        } else if (data.type === "refreshStatus") {
          setRefreshStatus(data.status === "running" ? "running" : "idle");
        }
      };

      ws.onclose = () => {
        wsRef.current = null;
        if (retriesRef.current >= WS_MAX_RETRIES) return;
        const delay = Math.min(
          WS_RECONNECT_BASE_MS * Math.pow(2, retriesRef.current),
          WS_RECONNECT_MAX_MS,
        );
        retriesRef.current += 1;
        reconnectTimeoutRef.current = globalThis.setTimeout(connect, delay);
      };
    };

    connect();

    return () => {
      if (reconnectTimeoutRef.current) {
        globalThis.clearTimeout(reconnectTimeoutRef.current);
      }
      wsRef.current?.close();
    };
  }, [resolvedUrl, queryClient]);
}