import { useState, useCallback, useRef } from "react";
import { BASE_URL } from "../api/client";
import type { SSEStreamEvent, BrowserContext } from "../types";

export interface UseSSEOptions {
  onChunk?: (content: string) => void;
  onDone?: (messageId: string, userMessageId?: string, location?: string | null, tools_used?: string, user_location?: string, user_created_at?: string) => void;
  onToolCall?: (name: string, args: unknown) => void;
  onToolResult?: (name: string, success: boolean) => void;
  onWidget?: (id: string, name: string, data: unknown) => void;
  onError?: (message: string) => void;
  onApprovalRequired?: (
    requestId: string,
    toolName: string,
    reason: string,
  ) => void;
  onApprovalResult?: (requestId: string, approved: boolean) => void;
}

export function useSSE() {
  const [connected, setConnected] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  const connect = useCallback(
    async (
      content: string,
      options: UseSSEOptions = {},
      browserContext?: BrowserContext,
    ) => {
      if (abortRef.current) {
        abortRef.current.abort();
      }
      abortRef.current = new AbortController();

      setConnected(true);
      console.log(
        "[useSSE] Connecting to:",
        `${BASE_URL}/chat/stream`,
        "body content length:",
        content.length,
      );

      try {
        const body: Record<string, unknown> = { content };
        if (browserContext) {
          body.browser_context = browserContext;
        }

        const response = await fetch(`${BASE_URL}/chat/stream`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
          signal: abortRef.current.signal,
        });

        if (!response.ok) {
          console.error(
            "[useSSE] Response not OK:",
            response.status,
            response.statusText,
          );
          const errorBody = await response.json().catch(() => null);
          options.onError?.(errorBody?.error || `HTTP ${response.status}`);
          setConnected(false);
          return;
        }

        const reader = response.body?.getReader();
        if (!reader) {
          console.error("[useSSE] No response body reader");
          options.onError?.("No response body");
          setConnected(false);
          return;
        }

        const decoder = new TextDecoder();
        let buffer = "";

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split("\n");
          buffer = lines.pop() || "";

          for (const line of lines) {
            if (line.startsWith("data: ")) {
              try {
                const event: SSEStreamEvent = JSON.parse(line.slice(6));

                console.log("[useSSE] Event received:", event.type, event);

                switch (event.type) {
                  case "chunk":
                    options.onChunk?.(event.content || "");
                    break;
                  case "done":
                    options.onDone?.(
                      event.message_id || "",
                      event.user_message_id,
                      event.location,
                      event.tools_used,
                      event.user_location,
                      event.user_created_at,
                    );
                    break;
                  case "error":
                    options.onError?.(event.message || "Unknown error");
                    break;
                  case "tool_call":
                    options.onToolCall?.(event.name || "", event.args);
                    break;
                  case "tool_result":
                    options.onToolResult?.(
                      event.name || "",
                      event.success ?? false,
                    );
                    break;
                  case "widget":
                    if (!event.id) {
                      console.warn(
                        "[useSSE] Ignoring widget event without id",
                        event,
                      );
                      break;
                    }
                    options.onWidget?.(event.id, event.name || "", event.data);
                    break;
                  case "approval_required":
                    options.onApprovalRequired?.(
                      event.request_id || "",
                      event.tool_name || "",
                      event.reason || "",
                    );
                    break;
                  case "approval_result":
                    options.onApprovalResult?.(
                      event.request_id || "",
                      event.approved ?? false,
                    );
                    break;
                  default:
                    break;
                }
              } catch {
                console.warn("[useSSE] Failed to parse SSE line:", line);
                // Skip malformed JSON lines
              }
            }
          }
        }
      } catch (err) {
        console.error("[useSSE] Fetch error:", (err as Error).message);
        if ((err as Error).name !== "AbortError") {
          options.onError?.((err as Error).message);
        }
      } finally {
        console.log("[useSSE] Connection closed");
        setConnected(false);
      }
    },
    [],
  );

  const disconnect = useCallback(() => {
    console.log("[useSSE] Disconnecting");
    if (abortRef.current) {
      abortRef.current.abort();
      abortRef.current = null;
    }
    setConnected(false);
  }, []);

  return { connect, disconnect, connected };
}
