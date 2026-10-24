import { useState, useCallback, useEffect, useRef } from "react";
import type { Message } from "../types";
import { api } from "../api/client";
import { useSSE } from "./useSSE";
import { useChatSync } from "./useChatSync";
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
 * Identificador que el cliente propone para su mensaje de usuario
 * (`client_message_id`). El servidor lo usa tal cual si es una cadena no vacía
 * de hasta 64 caracteres, así que la burbuja optimista y la fila canónica
 * comparten id y la sustitución es por id, no por contenido.
 */
function newClientMessageId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `cli-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
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
  /**
   * Texto de un envío rechazado con `409` (el chat está ocupado por un turno
   * ajeno). El servidor no persistió el mensaje de usuario, así que la burbuja
   * optimista se retira **y el texto vuelve a estar disponible para
   * reenviarlo** en vez de perderse. Se limpia al empezar el envío siguiente.
   */
  const [rejectedDraft, setRejectedDraft] = useState<string | null>(null);
  /**
   * `true` mientras el `busy` que reporta el bus corresponde a **mi** turno.
   *
   * El bus publica el fin de turno por un canal **distinto** al `done` del
   * stream: entre ambos el `busy` sigue en `true`. Mantenerlo marcado hasta que
   * el bus se declare libre evita que el dispositivo que originó el turno
   * muestre «Valet está respondiendo en otro dispositivo» durante ese hueco.
   */
  const [ownBusy, setOwnBusy] = useState(false);
  // Ref (no estado) para consultar desde callbacks sin depender del ciclo de render.
  const streamingRef = useRef(false);
  /**
   * Turno **propio** en vuelo: activo desde el envío hasta `done`/`onError`. Con
   * él se distingue el `busy` del bus que proviene de mi propio turno.
   */
  const ownTurnRef = useRef(false);
  /** `false` tras el desmontaje: los callbacks async no deben tocar estado. */
  const mountedRef = useRef(true);
  /**
   * Puente para que la suscripción pida una relectura del historial sin que el
   * motor de `useChatSync` conozca `chatInit` (mantiene el desacoplamiento).
   * `reloadHistory` se declara más abajo, así que se pasa por ref espejo.
   */
  const loadHistoryRef = useRef<(() => void) | null>(null);
  const requestHistoryReload = useCallback(() => {
    loadHistoryRef.current?.();
  }, []);

  /**
   * Transiciones del `busy` que reporta el bus. Es **mío** si el turno propio
   * seguía en vuelo cuando el bus lo anunció; cuando el bus se declara libre
   * deja de serlo. Se actualiza desde el callback de la suscripción —no desde
   * un efecto— para no leer refs durante el render ni encadenar renders.
   */
  const handleBusyChange = useCallback((value: boolean) => {
    if (value) {
      if (ownTurnRef.current) setOwnBusy(true);
    } else {
      setOwnBusy(false);
    }
  }, []);

  // La lista de mensajes la posee `useChatSync`: es la única fuente, y la
  // suscripción al bus de eventos la abre al montar, **al margen** de la carga
  // del historial. Un upsert por id hace que el resultado converja; además, la
  // suscripción pide `requestHistoryReload` cuando abre **sin cursor**, para
  // que ningún mensaje caiga en el hueco entre suscribir al bus y leer el
  // historial.
  const sync = useChatSync({
    onCursorlessOpen: requestHistoryReload,
    onBusyChange: handleBusyChange,
  });
  // Se desestructuran para que las dependencias de los `useCallback` sean los
  // miembros estables del handle y no el objeto —nuevo en cada render—.
  const { upsert, upsertMany, remove } = sync;

  const sse = useSSE();

  /**
   * Aplica al estado un historial ya leído: fusión por **upsert de id**
   * (idempotente) y reconstrucción de los widgets persistidos por mensaje. Los
   * mensajes sin widgets no crean entrada. Los widgets se **fusionan**, no se
   * reemplazan: en una recarga no deben borrarse los widgets vivos del turno en
   * curso (clave `"streaming"`) ni los de mensajes recién llegados por el bus.
   */
  const applyHistory = useCallback(
    (history: Message[]) => {
      upsertMany(history);
      const widgets: Record<string, WidgetInstance[]> = {};
      for (const message of history) {
        if (message.widgets && message.widgets.length > 0) {
          widgets[message.id] = message.widgets;
        }
      }
      setWidgetsByMessage((prev) => ({ ...prev, ...widgets }));
      console.log(
        "[useMainChat] Chat history loaded, messages:",
        history.length,
      );
    },
    [upsertMany],
  );

  /**
   * Relectura **de fondo** del historial, la que pide la suscripción al abrir sin
   * cursor. No toca `loading` ni pinta `error`: lo que el usuario está viendo
   * sigue siendo válido y un fallo aquí no debe sustituirlo.
   */
  const reloadHistory = useCallback(async () => {
    try {
      const data = await api.chatInit();
      if (!mountedRef.current) return;
      applyHistory(data.messages || []);
    } catch (err) {
      console.error(
        "[useMainChat] Failed to reload chat history:",
        (err as Error).message,
      );
    }
  }, [applyHistory]);

  useEffect(() => {
    mountedRef.current = true;
    loadHistoryRef.current = () => {
      void reloadHistory();
    };
    return () => {
      mountedRef.current = false;
    };
  }, [reloadHistory]);

  useEffect(() => {
    // Carga **inicial**: único sitio donde vive `setLoading`, apagado en el
    // `finally` del encadenado para que un fallo no deje el spinner colgado.
    api
      .chatInit()
      .then((data) => {
        if (!mountedRef.current) return;
        applyHistory(data.messages || []);
      })
      .catch((err: Error) => {
        console.error(
          "[useMainChat] Failed to initialize chat:",
          err.message,
        );
        if (mountedRef.current) setError(err.message);
      })
      .finally(() => {
        if (mountedRef.current) setLoading(false);
      });
  }, [applyHistory]);

  const sendMessage = useCallback(
    async (content: string) => {
      console.log("[useMainChat] Sending message:", content.slice(0, 100));

      // El id lo propone el cliente y viaja al servidor como
      // `client_message_id`: la fila canónica llegará con este mismo id.
      const clientMessageId = newClientMessageId();

      const optimistic: Message = {
        id: clientMessageId,
        role: "user",
        content,
        created_at: new Date().toISOString(),
      };

      upsert(optimistic, { pending: true });
      streamingRef.current = true;
      // El turno es mío: si el bus confirma que hay un turno en curso, es éste.
      ownTurnRef.current = true;
      setStreaming(true);
      setStreamingContent("");
      setError(null);
      setRejectedDraft(null);
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
            // La fila del mensaje de usuario adopta la marca temporal del
            // servidor cuando llega; si el servidor no la manda (o propuso otro
            // id), se conserva la burbuja optimista sin duplicarla.
            if (userMessageId && userMessageId !== clientMessageId) {
              remove(clientMessageId);
            }
            // Sin `user_created_at` no hay marca del servidor todavía: la fila
            // queda optimista y su hora no pisa la de una fila ya recibida del
            // bus.
            upsert(
              {
                id: userMessageId || clientMessageId,
                role: "user",
                content,
                location: user_location ?? null,
                created_at: user_created_at
                  ? user_created_at
                  : optimistic.created_at,
              },
              { pending: !user_created_at },
            );
            // El mensaje del asistente se pinta en cuanto termina el turno, pero
            // su marca temporal es la del navegador hasta que llegue su fila
            // canónica por el bus (que la sustituirá, porque va marcado como
            // optimista y una fila del servidor nunca es pisada por una local).
            upsert(
              {
                id: assistantId,
                role: "assistant",
                content: assistantContent,
                location: location || null,
                tools_used: tools_used || undefined,
                created_at: new Date().toISOString(),
              },
              { pending: true },
            );
            streamingRef.current = false;
            // Mi turno termina aquí; el `busy` que siga reportando el bus hasta
            // su `turn_finished` ya está marcado como propio por `ownBusy`.
            ownTurnRef.current = false;
            setStreaming(false);
            setStreamingContent("");
            setActiveTools([]);
            setUsedTools([]);
            setPendingApproval(null);
          },
          onError: (msg, status, partial) => {
            console.error("[useMainChat] Stream error:", msg, status);
            setError(msg);
            if (status === 409) {
              // El turno no salió: el servidor no persistió el mensaje de
              // usuario, así que la burbuja optimista no puede quedarse como si
              // se hubiera enviado...
              remove(clientMessageId);
              // ...y el texto vuelve a estar disponible para reenviarlo cuando
              // el turno ajeno termine, en vez de perderse.
              setRejectedDraft(content);
            } else if (partial && partial.trim() !== "") {
              // El stream falló a mitad, pero ya se había pintado texto: NO se
              // descarta. Se persiste como burbuja de asistente marcada como
              // interrumpida para que el parcial siga visible tras el fallo. No
              // se persiste en el servidor (diseño D3): una recarga lo pierde.
              upsert(
                {
                  id: `msg-${Date.now()}`,
                  role: "assistant",
                  content: partial,
                  interrupted: true,
                  created_at: new Date().toISOString(),
                },
                { pending: true },
              );
            }
            streamingRef.current = false;
            // El turno propio no llegó a salir (o falló): cualquier `busy` que
            // reporte el bus es de otro dispositivo y debe verse como ajeno.
            ownTurnRef.current = false;
            setOwnBusy(false);
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
        clientMessageId,
      );
    },
    [sse, upsert, remove],
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
    messages: sync.messages,
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
    // Turno en curso **ajeno**: se descuenta el turno propio dos veces —el que
    // ya se ve en `streaming` y el que el bus sigue marcando como ocupado entre
    // el `done` del stream propio y su `turn_finished`—.
    remoteBusy: sync.busy && !streaming && !ownBusy,
    // Texto de un envío rechazado con `409`, listo para reponer en el campo.
    rejectedDraft,
  };
}
