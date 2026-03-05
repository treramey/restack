import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "./queries.js";

type WsEvent = {
  type: "invalidate";
  queryKeys: string[][];
};

export function useWebSocketSync(url: string = `ws://${globalThis.location.host}/ws`) {
  const queryClient = useQueryClient();
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    const connect = () => {
      const ws = new WebSocket(url);

      ws.onopen = () => {
        wsRef.current = ws;
      };

      ws.onmessage = (event) => {
        try {
          const data: WsEvent = JSON.parse(event.data);
          if (data.type === "invalidate" && Array.isArray(data.queryKeys)) {
            for (const key of data.queryKeys) {
              const rootKey = key[0];
              if (rootKey && rootKey in queryKeys) {
                queryClient.invalidateQueries({
                  queryKey: queryKeys[rootKey as keyof typeof queryKeys].all,
                });
              }
            }
          }
        } catch {}
      };

      ws.onclose = () => {
        wsRef.current = null;
        reconnectTimeoutRef.current = globalThis.setTimeout(connect, 3000);
      };
    };

    connect();

    return () => {
      if (reconnectTimeoutRef.current) {
        globalThis.clearTimeout(reconnectTimeoutRef.current);
      }
      wsRef.current?.close();
    };
  }, [url, queryClient]);
}