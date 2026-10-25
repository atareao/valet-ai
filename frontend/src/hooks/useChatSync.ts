import { useCallback, useEffect, useRef, useState } from "react";
import { UNAUTHORIZED_EVENT, chatEventsUrl } from "../api/client";
import type { ChatEvent, ChatCursor, Message } from "../types";
import { upsertMessage, upsertMessages } from "../utils/chatMessages";

/**
 * Sincronización del chat entre dispositivos (change `chat-cross-device-sync`).
 *
 * Mantiene **la lista de mensajes** del hilo —única fuente— y la mantiene al día
 * con el bus de eventos `GET /api/chat/events`:
 *
 * - fusión por **upsert de id**, idempotente, de modo que recibir el mismo
 *   mensaje por el stream del turno propio y por el bus no lo duplica;
 * - orden por `(created_at, id)` con la marca temporal del **servidor**;
 * - la condición de **optimista** viaja en el propio mensaje (`Message.pending`,
 *   change `chat-pending-flag-leak`): una fila canónica la retira al fusionarse
 *   y una escritura optimista sobre una fila ya canónica se descarta, de modo
 *   que ninguna fila del servidor queda marcada;
 * - recuperación al volver a la pestaña (relectura por `since` + reapertura) y
 *   reintento con retroceso exponencial (1 s → 30 s);
 * - un `401` dispara el evento global de sesión caducada y **no** reintenta.
 *
 * Cuando una apertura va **sin cursor** (arranque, reintento antes del primer
 * `message_created` o cursor rechazado con `400`), el servidor no hace replay;
 * el motor pide entonces una relectura del historial con `onCursorlessOpen`, y
 * la pide **después** de que la respuesta del stream se haya establecido (es
 * decir, con el servidor ya suscrito al bus), que es lo que cierra el hueco.
 *
 * La suscripción se abre al montar y se cierra al desmontar.
 */

/** Tope del retroceso exponencial entre reintentos de suscripción. */
export const CHAT_SYNC_MAX_RETRY_MS = 30_000;

/** Base del retroceso exponencial: 1 s, 2 s, 4 s… (spec: «retroceso exponencial»). */
const CHAT_SYNC_BASE_RETRY_MS = 1_000;

/** Retardo del intento número `attempt` (0 = primero), con tope de 30 s. */
export function retryDelayMs(attempt: number): number {
  const exponential = CHAT_SYNC_BASE_RETRY_MS * 2 ** Math.max(0, attempt);
  return Math.min(exponential, CHAT_SYNC_MAX_RETRY_MS);
}

export interface ChatSyncHandle {
  /** Lista fusionada y ordenada por `(created_at, id)`. */
  messages: Message[];
  /**
   * Inserta o sustituye un mensaje por su id y reordena la lista.
   *
   * `options.pending` marca la fila como **optimista** (todavía sin fila
   * canónica): se coloca al final de la lista hasta que llegue su versión del
   * servidor y, si esa versión ya está, la escritura optimista **se descarta**
   * para no pisar lo que dijo el servidor. La marca viaja en el propio mensaje
   * (`Message.pending`, change `chat-pending-flag-leak`), no en un estado aparte:
   * una fila canónica nunca la conserva.
   */
  upsert: (message: Message, options?: { pending?: boolean }) => void;
  /** Inserta o sustituye varios mensajes de una vez (carga inicial). */
  upsertMany: (messages: Message[]) => void;
  /** Quita un mensaje de la lista (un envío que no llegó a persistirse). */
  remove: (id: string) => void;
  /**
   * `true` mientras el bus informa de un turno en curso (propio o ajeno). Quien
   * lo pinte como «otro dispositivo» debe descontar su propio turno.
   */
  busy: boolean;
}

interface ChatEventsHandlers {
  onMessage: (message: Message, cursor: ChatCursor) => void;
  onBusyChange: (busy: boolean) => void;
  /**
   * Se llama al **terminar de establecerse** una apertura del stream que fue
   * **sin cursor** (`since` ausente). Es el momento en que el servidor ya
   * suscribió al bus, así que releer el historial aquí no deja hueco: un
   * mensaje persistido en el intervalo está o en el historial o en el flujo en
   * vivo. El motor no conoce `chatInit`: quien lo monta decide qué releer.
   */
  onCursorlessOpen?: () => void;
}

interface ChatEventsSubscription {
  start: () => void;
  stop: () => void;
  /** Relee y reabre la suscripción ahora mismo, con el último cursor. */
  recover: () => void;
}

/**
 * Motor de la suscripción, fuera del ciclo de render: el estado mutable (cursor,
 * intento, temporizador, controlador) vive en el cierre y los avisos salen por
 * `handlers`. Se apoya en `fetch` + reader —igual que `useSSE`— en vez de
 * `EventSource`, para poder mirar el estado HTTP (el `401` dispara el evento
 * global de sesión caducada) y porque `EventSource` no admite cabeceras.
 */
function createChatEventsSubscription(
  handlers: ChatEventsHandlers,
): ChatEventsSubscription {
  let cursor: ChatCursor | null = null;
  let controller: AbortController | null = null;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  let attempt = 0;
  let stopped = true;
  let resyncRequested = false;
  let visibilityListener: (() => void) | null = null;

  const clearRetry = () => {
    if (retryTimer !== null) {
      clearTimeout(retryTimer);
      retryTimer = null;
    }
  };

  const handleEvent = (raw: string) => {
    let event: ChatEvent;
    try {
      event = JSON.parse(raw) as ChatEvent;
    } catch {
      console.warn("[useChatSync] Evento ilegible, se descarta:", raw);
      return;
    }

    switch (event.type) {
      case "snapshot":
        handlers.onBusyChange(event.busy === true);
        break;
      case "message_created":
        if (!event.message || !event.cursor) {
          console.warn(
            "[useChatSync] message_created sin mensaje o sin cursor:",
            event,
          );
          break;
        }
        // El cursor avanza con el último mensaje visto: es lo que se manda como
        // `since` al reabrir, y por eso el replay no repite lo ya visto.
        cursor = event.cursor;
        handlers.onMessage(event.message, event.cursor);
        break;
      case "turn_started":
        handlers.onBusyChange(true);
        break;
      case "turn_finished":
        handlers.onBusyChange(false);
        break;
      case "resync":
        // El servidor cierra el stream tras avisar: se reabre con el cursor.
        resyncRequested = true;
        break;
      default:
        console.warn("[useChatSync] Evento desconocido, se ignora:", event);
        break;
    }
  };

  const scheduleRetry = () => {
    if (stopped || retryTimer !== null) return;
    const delay = retryDelayMs(attempt);
    attempt += 1;
    retryTimer = setTimeout(() => {
      retryTimer = null;
      void open();
    }, delay);
  };

  async function open(): Promise<void> {
    if (stopped) return;
    clearRetry();
    controller?.abort();

    const current = new AbortController();
    controller = current;
    resyncRequested = false;

    try {
      const response = await fetch(chatEventsUrl(cursor), {
        credentials: "include",
        headers: { Accept: "text/event-stream" },
        signal: current.signal,
      });

      if (!response.ok) {
        if (response.status === 401) {
          // Igual que el resto del cliente HTTP: el 401 corta la sesión, y
          // reintentar en bucle solo llenaría los logs.
          console.error("[useChatSync] Sesión caducada en la suscripción");
          window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
          return;
        }
        if (response.status === 400) {
          // Cursor ilegible (p. ej. formato anterior): se olvida y se vuelve a
          // conectar desde el principio en vez de reintentar con él.
          console.warn("[useChatSync] Cursor rechazado; se reabre sin `since`");
          cursor = null;
        } else {
          console.warn(
            "[useChatSync] Suscripción rechazada:",
            response.status,
            response.statusText,
          );
        }
        scheduleRetry();
        return;
      }

      // El servidor atendió: el retroceso vuelve a empezar por 1 s.
      attempt = 0;

      // Sin cursor, la URL fue **sin `since`** y el servidor no hace replay
      // (solo `snapshot` + flujo en vivo): lo persistido antes de la
      // suscripción no está en el flujo, así que se relee el historial entero.
      // Se hace **ahora**, con la respuesta ya establecida, porque es la
      // resolución del `fetch` la que garantiza que el servidor suscribió al
      // bus antes de leer; hacerlo antes dejaría abierto el hueco que este
      // change existe para cerrar. Cubre el arranque, la reapertura sin cursor
      // y la rama que olvida un cursor rechazado.
      if (cursor === null) {
        handlers.onCursorlessOpen?.();
      }

      const reader = response.body?.getReader();
      if (!reader) {
        console.warn("[useChatSync] La respuesta no trae cuerpo legible");
        scheduleRetry();
        return;
      }

      const decoder = new TextDecoder();
      let buffer = "";

      while (!stopped) {
        const { done, value } = await reader.read();
        if (done || stopped) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        // El último trozo puede estar a medias: se guarda para el siguiente.
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          // Los latidos son comentarios SSE (`:`): no son eventos y se ignoran.
          if (!line.startsWith("data:")) continue;
          handleEvent(line.slice(5).trim());
          if (resyncRequested) break;
        }

        if (resyncRequested) {
          await reader.cancel().catch(() => undefined);
          break;
        }
      }
    } catch (err) {
      // Un aborto propio (desmontaje o reapertura) no es un fallo: no reintenta.
      if (current.signal.aborted) return;
      console.warn(
        "[useChatSync] Fallo de la suscripción:",
        (err as Error).message,
      );
      scheduleRetry();
      return;
    }

    if (stopped) return;

    if (resyncRequested) {
      // Reapertura inmediata, con el último cursor conocido.
      resyncRequested = false;
      void open();
      return;
    }

    // El stream terminó solo (red caída, servidor o cierre): reintentar.
    scheduleRetry();
  }

  const recover = () => {
    if (stopped) return;
    // Recuperación inmediata: no se espera el retroceso pendiente.
    attempt = 0;
    clearRetry();
    void open();
  };

  const handleVisibilityChange = () => {
    if (document.visibilityState !== "visible") return;
    recover();
  };

  return {
    start: () => {
      if (!stopped) return;
      stopped = false;
      attempt = 0;
      visibilityListener = handleVisibilityChange;
      document.addEventListener("visibilitychange", visibilityListener);
      void open();
    },
    stop: () => {
      stopped = true;
      clearRetry();
      if (visibilityListener) {
        document.removeEventListener("visibilitychange", visibilityListener);
        visibilityListener = null;
      }
      controller?.abort();
      controller = null;
    },
    recover,
  };
}

export interface ChatSyncOptions {
  /**
   * Relectura del historial pedida por el motor cuando la suscripción se abre
   * **sin cursor** (ver [ChatEventsHandlers.onCursorlessOpen]). El motor no
   * conoce `chatInit`: se lo pasa quien lo monta.
   */
  onCursorlessOpen?: () => void;
  /**
   * Transiciones del `busy` del bus, **además** de quedar reflejadas en el
   * handle. Permite a quien monta el motor reconocer el `busy` que proviene de
   * su propio turno (ver [ChatSyncHandle.busy]).
   */
  onBusyChange?: (busy: boolean) => void;
}

export function useChatSync(options?: ChatSyncOptions): ChatSyncHandle {
  const [messages, setMessages] = useState<Message[]>([]);
  const [busy, setBusy] = useState(false);
  // Refs espejo de los callbacks: la suscripción vive en un `useEffect([])` y no
  // debe recrearse cuando cambie la identidad de los callbacks.
  const onCursorlessOpenRef = useRef(options?.onCursorlessOpen);
  const onBusyChangeRef = useRef(options?.onBusyChange);

  useEffect(() => {
    onCursorlessOpenRef.current = options?.onCursorlessOpen;
  }, [options?.onCursorlessOpen]);

  useEffect(() => {
    onBusyChangeRef.current = options?.onBusyChange;
  }, [options?.onBusyChange]);

  const upsert = useCallback(
    (message: Message, options?: { pending?: boolean }) => {
      // La marca de optimista viaja en el mensaje (design D1): se adjunta aquí
      // y la fusión se encarga de retirarla cuando llegue la fila canónica.
      const row: Message =
        options?.pending === true ? { ...message, pending: true } : message;
      setMessages((prev) => upsertMessage(prev, row));
    },
    [],
  );

  const upsertMany = useCallback((incoming: Message[]) => {
    // El historial puede venir vacío (recarga de un hilo sin mensajes): no hay
    // nada que fusionar y se evita un reordenado estéril.
    if (incoming.length === 0) return;
    setMessages((prev) => upsertMessages(prev, incoming));
  }, []);

  const remove = useCallback((id: string) => {
    setMessages((prev) => prev.filter((message) => message.id !== id));
  }, []);

  useEffect(() => {
    const subscription = createChatEventsSubscription({
      onMessage: (message) => {
        // Un mensaje que llega por el bus ya está persistido: la fusión retira
        // la marca de optimista si la hubiera, porque el mensaje entrante no la
        // trae (design D1/D3).
        setMessages((prev) => upsertMessage(prev, message));
      },
      onBusyChange: (value) => {
        setBusy(value);
        onBusyChangeRef.current?.(value);
      },
      onCursorlessOpen: () => onCursorlessOpenRef.current?.(),
    });

    subscription.start();
    return () => subscription.stop();
  }, []);

  return { messages, upsert, upsertMany, remove, busy };
}
