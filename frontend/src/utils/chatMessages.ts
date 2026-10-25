import type { Message } from "../types";
import { parseSqliteTimestamp } from "./sqliteTimestamp";

/**
 * Fusión de la lista de mensajes del chat (change `chat-cross-device-sync`).
 *
 * La lista tiene **dos fuentes**: el historial del turno propio (`chatInit`, el
 * evento `done` y la burbuja optimista) y el bus de eventos
 * (`GET /api/chat/events`). Como el evento canónico es la fila persistida
 * (design D2), la fusión es un **upsert por id** y el orden relativo entre
 * ambas fuentes deja de importar: el resultado converge.
 *
 * La condición de «optimista» viaja **en el propio mensaje** (`Message.pending`,
 * change `chat-pending-flag-leak`), no en un `Set` de identificadores aparte:
 * así una fila canónica nunca puede conservar la marca (design D1).
 */

/**
 * Normaliza la marca temporal que sirve el backend a ISO-8601 **UTC**,
 * preservando el instante.
 *
 * El crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`, UTC) no es ISO-8601: `new Date()`
 * lo interpreta como hora **local**, de modo que dos dispositivos mostrarían
 * instantes distintos del mismo mensaje y la ordenación mezclaría dos formatos.
 * Normalizar en la frontera deja una sola forma canónica en la lista.
 *
 * Lo que no parsea se devuelve tal cual: no se inventa una fecha (equivale al
 * contrato de `parseSqliteTimestamp`).
 */
export function normalizeServerTimestamp(value: string): string {
  const instant = parseSqliteTimestamp(value);
  return instant ? instant.toISOString() : value;
}

/** Aplica [normalizeServerTimestamp] a la marca temporal del mensaje. */
export function normalizeMessage(message: Message): Message {
  return { ...message, created_at: normalizeServerTimestamp(message.created_at) };
}

/**
 * Instante de un mensaje para ordenar. Lo que no parsea vale `0`: queda delante
 * de todo lo demás y, entre sí, lo desempata el id. Es determinista.
 */
function instantOf(message: Message): number {
  return parseSqliteTimestamp(message.created_at)?.getTime() ?? 0;
}

/**
 * Orden total por la pareja del servidor `(created_at, id)` (design D3/D4).
 *
 * El desempate por `id` es una comparación de bytes, la misma que aplica SQLite
 * sobre la columna. Un mensaje **pendiente** (optimista, sin fila canónica
 * todavía) va al final de la lista hasta que su fila llegue y lo sustituya: su
 * marca temporal es la del reloj del navegador y no debe colocarlo en medio del
 * historial. La marca viaja en el propio mensaje (`Message.pending`), no en un
 * `Set` de identificadores aparte (design D1).
 */
export function sortMessages(messages: readonly Message[]): Message[] {
  return [...messages].sort((a, b) => {
    const aPending = a.pending === true;
    const bPending = b.pending === true;
    if (aPending !== bPending) return aPending ? 1 : -1;

    const diff = instantOf(a) - instantOf(b);
    if (diff !== 0) return diff;

    if (a.id === b.id) return 0;
    return a.id < b.id ? -1 : 1;
  });
}

/**
 * Upsert por id en lote seguido de **un único** reordenado.
 *
 * Es la forma que usa un replay: fusionar N mensajes uno a uno reordena N veces
 * —O(N² log N)— y el replay del bus puede llegar de golpe. Con un `Map` por id
 * y un solo [sortMessages] al final, el coste es O(N log N) y el resultado es
 * idéntico: cada id repetido dentro del propio lote se queda con la última fila.
 *
 * La fusión es **idempotente** —la misma fila dos veces deja la lista igual—,
 * así que recibir un mensaje por el stream del turno propio y por el bus de
 * eventos no lo duplica.
 *
 * La condición de «optimista» viaja en el propio mensaje (`Message.pending`,
 * design D1): una escritura optimista (`pending: true`) no pisa una fila
 * canónica ya recibida (design D2) y una fila canónica (sin la marca) **retira**
 * la marca de una optimista (design D3), de modo que ninguna fila del servidor
 * la conserva.
 */
export function upsertMessages(
  current: readonly Message[],
  incoming: readonly Message[],
): Message[] {
  const byId = new Map<string, Message>(
    current.map((message) => [message.id, message]),
  );

  for (const message of incoming) {
    const normalized = normalizeMessage(message);
    const existing = byId.get(normalized.id);

    // D2: una escritura optimista no pisa una fila canónica ya recibida.
    // Se descarta la entrante y la fila del servidor se conserva intacta.
    if (normalized.pending === true && existing && existing.pending !== true) {
      continue;
    }

    if (!existing) {
      byId.set(normalized.id, normalized);
      continue;
    }

    const merged: Message = { ...existing, ...normalized };
    // D3: una fila canónica (sin la marca) retira la marca de una optimista.
    if (normalized.pending !== true) delete merged.pending;
    byId.set(normalized.id, merged);
  }

  return sortMessages([...byId.values()]);
}

/**
 * Upsert por id de un solo mensaje: si el id ya está en la lista se sustituye la
 * fila, y si no, se inserta.
 */
export function upsertMessage(
  current: readonly Message[],
  incoming: Message,
): Message[] {
  return upsertMessages(current, [incoming]);
}
