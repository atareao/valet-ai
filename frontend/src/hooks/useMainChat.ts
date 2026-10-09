import { useState, useCallback, useEffect, useRef } from "react";
import type { Message } from "../types";
import { api } from "../api/client";
import { useSSE } from "./useSSE";
import type { WidgetInstance } from "../components/widgets/types";
import { formatWidgetAction } from "../components/widgets/actions";

function getTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return "UTC";
  }
}

function getTimestamp(): string {
  return new Date().toISOString();
}

/**
 * Get the browser's geolocation as a Promise.
 * Falls back to null if unavailable, denied, or timed out.
 */
function getCurrentPosition(): Promise<{
  latitude: number;
  longitude: number;
} | null> {
  return new Promise((resolve) => {
    if (!navigator.geolocation) {
      resolve(null);
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (pos) =>
        resolve({
          latitude: pos.coords.latitude,
          longitude: pos.coords.longitude,
        }),
      () => resolve(null),
      { enableHighAccuracy: false, timeout: 5000, maximumAge: 300000 },
    );
  });
}

export function useMainChat() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [streamingContent, setStreamingContent] = useState<string>("");
  const [streaming, setStreaming] = useState(false);
  const [activeTools, setActiveTools] = useState<string[]>([]);
  const [usedTools, setUsedTools] = useState<string[]>([]);
  const [pendingApproval, setPendingApproval] = useState<{
    requestId: string;
    toolName: string;
    reason: string;
  } | null>(null);
  const [widgetsByMessage, setWidgetsByMessage] = useState<
    Record<string, WidgetInstance[]>
  >({});
  // Ref (no estado) para consultar desde callbacks sin depender del ciclo de render.
  const streamingRef = useRef(false);

  const sse = useSSE();

  useEffect(() => {
    let mounted = true;

    api
      .chatInit()
      .then((data) => {
        if (!mounted) return;
        const history = data.messages || [];
        setMessages(history);
        // Reconstruye los widgets persistidos en cada mensaje del historial para
        // que vuelvan a mostrarse tras recargar la página. Los mensajes sin
        // widgets no crean entrada.
        const initialWidgets: Record<string, WidgetInstance[]> = {};
        for (const message of history) {
          if (message.widgets && message.widgets.length > 0) {
            initialWidgets[message.id] = message.widgets;
          }
        }
        setWidgetsByMessage(initialWidgets);
        console.log(
          "[useMainChat] Chat initialized, messages:",
          history.length,
        );
      })
      .catch((err: Error) => {
        console.error("[useMainChat] Failed to initialize chat:", err.message);
        if (mounted) setError(err.message);
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });

    return () => {
      mounted = false;
    };
  }, []);

  const sendMessage = useCallback(
    async (content: string) => {
      console.log("[useMainChat] Sending message:", content.slice(0, 100));

      const optimistic: Message = {
        id: "temp-" + Date.now(),
        role: "user",
        content,
        created_at: new Date().toISOString(),
      };

      setMessages((prev) => [...prev, optimistic]);
      streamingRef.current = true;
      setStreaming(true);
      setStreamingContent("");
      setError(null);
      setActiveTools([]);
      setUsedTools([]);
      setPendingApproval(null);

      let assistantContent = "";

      // Get browser context: timestamp + timezone
      const browserContext = {
        timestamp: getTimestamp(),
        timezone: getTimezone(),
        latitude: null as number | null,
        longitude: null as number | null,
        location_name: null as string | null,
      };

      // Try to get the browser's geolocation (falls back to null on denial/timeout)
      const coords = await getCurrentPosition();
      if (coords) {
        browserContext.latitude = coords.latitude;
        browserContext.longitude = coords.longitude;
        console.log(
          "[useMainChat] Geolocation obtained:",
          coords.latitude,
          coords.longitude,
        );
      } else {
        console.warn("[useMainChat] Geolocation unavailable or denied");
      }

      sse.connect(
        content,
        {
          onChunk: (chunk) => {
            console.log("[useMainChat] Chunk received:", chunk.slice(0, 50));
            assistantContent += chunk;
            setStreamingContent(assistantContent);
          },
          onToolCall: (name: string) => {
            console.log("[useMainChat] Tool call:", name);
            setActiveTools((prev) => [...prev, name]);
            setUsedTools((prev) =>
              prev.includes(name) ? prev : [...prev, name],
            );
          },
          onToolResult: (name: string, success: boolean) => {
            console.log(
              "[useMainChat] Tool result:",
              name,
              "success:",
              success,
            );
            // Notify CalendarView when LLM finishes a calendar operation
            if (name === "calendar" && success) {
              window.dispatchEvent(new CustomEvent("events-changed"));
            }
            // Notify TaskView when LLM completes a tasks operation
            if (name === "tasks" && success) {
              window.dispatchEvent(new CustomEvent("tasks-changed"));
            }
          },
          onWidget: (id: string, name: string, data: unknown) => {
            if (!streamingRef.current) {
              console.warn(
                "[useMainChat] Widget ignored (no active stream):",
                name,
                id,
              );
              return;
            }
            console.log("[useMainChat] Widget received:", name, id);
            const widget: WidgetInstance = { id, name, data };
            setWidgetsByMessage((prev) => ({
              ...prev,
              streaming: [...(prev["streaming"] ?? []), widget],
            }));
          },
          onApprovalRequired: (
            requestId: string,
            toolName: string,
            reason: string,
          ) => {
            console.log(
              "[useMainChat] Approval required:",
              requestId,
              toolName,
            );
            setPendingApproval({ requestId, toolName, reason });
          },
          onApprovalResult: (requestId: string) =>
            setPendingApproval((cur) =>
              cur && cur.requestId === requestId ? null : cur,
            ),
          onDone: (messageId: string, userMessageId?: string, location?: string | null, tools_used?: string, user_location?: string, user_created_at?: string) => {
            console.log(
              "[useMainChat] Stream done. Total content length:",
              assistantContent.length,
            );
            const assistantId = messageId || "msg-" + Date.now();
            // Move live widgets from the streaming placeholder to the final message
            setWidgetsByMessage((prev) => {
              const next = { ...prev };
              const streamingWidgets = next["streaming"];
              delete next["streaming"];
              if (streamingWidgets && streamingWidgets.length > 0) {
                next[assistantId] = streamingWidgets;
              }
              return next;
            });
            // Replace temp user message id with real one instead of removing it
            setMessages((prev) => {
              const assistant: Message = {
                id: assistantId,
                role: "assistant",
                content: assistantContent,
                location: location || null,
                tools_used: tools_used || undefined,
                created_at: new Date().toISOString(),
              };
              return [
                ...prev.map((m) =>
                  m.id === optimistic.id && userMessageId
                    ? { ...m, id: userMessageId, location: user_location ?? m.location, created_at: user_created_at ?? m.created_at }
                    : m,
                ),
                assistant,
              ];
            });
            streamingRef.current = false;
            setStreaming(false);
            setStreamingContent("");
            setActiveTools([]);
            setUsedTools([]);
            setPendingApproval(null);
          },
          onError: (msg) => {
            console.error("[useMainChat] Stream error:", msg);
            setError(msg);
            streamingRef.current = false;
            setStreaming(false);
            setStreamingContent("");
            setActiveTools([]);
            setUsedTools([]);
            setPendingApproval(null);
            // Discard live widgets that never made it into a finished message
            setWidgetsByMessage((prev) => {
              if (!("streaming" in prev)) return prev;
              const next = { ...prev };
              delete next["streaming"];
              return next;
            });
            // Keep the user message visible - DON'T filter it out
          },
        },
        browserContext,
      );
    },
    [sse],
  );

  const sendWidgetAction = useCallback(
    (widget: WidgetInstance, action: string, payload?: unknown) => {
      // Defensa: no abortar un stream en curso ni encolar un turno a medias.
      if (streamingRef.current) {
        console.warn("[useMainChat] Widget action ignored while streaming");
        return;
      }
      const content = formatWidgetAction(
        widget.name,
        widget.id,
        action,
        payload,
      );
      void sendMessage(content);
    },
    [sendMessage],
  );

  const resolveApproval = useCallback(
    async (approved: boolean) => {
      const current = pendingApproval;
      if (!current) return;
      try {
        await api.approveAction(current.requestId, approved);
        setPendingApproval(null);
      } catch (err) {
        console.error("[useMainChat] Failed to resolve approval:", err);
      }
    },
    [pendingApproval],
  );

  return {
    messages,
    loading,
    error,
    sendMessage,
    streaming,
    streamingContent,
    activeTools,
    usedTools,
    pendingApproval,
    resolveApproval,
    widgetsByMessage,
    sendWidgetAction,
  };
}
