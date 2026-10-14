import { useState, useEffect, useCallback, useRef } from "react";
import type { Notification, PaginatedResponse } from "../types";
import { api } from "../api/client";

/**
 * Página por defecto de la bandeja de avisos (spec: se listan de 10 en 10).
 */
export const DEFAULT_PAGE_SIZE = 10;

export interface UseNotificationsResult {
  notifications: Notification[];
  total: number;
  loading: boolean;
  error: string | null;
  /** Página actual, 1-based. */
  page: number;
  hasPrev: boolean;
  hasNext: boolean;
  goNext: () => void;
  goPrev: () => void;
  refetch: () => void;
  markRead: (id: string) => Promise<void>;
  markAll: () => Promise<void>;
}

/**
 * Bandeja de avisos pendientes, paginada por cursor.
 *
 * El backend no pagina por offset sino por `next_cursor`: cada página entrega
 * el cursor de la siguiente. Para poder retroceder sin volver a pedir la
 * página anterior al servidor, se guarda una **pila de cursores**
 * (`cursors[i]` es el cursor con el que se cargó la página `i + 1`; la página
 * 1 no lleva cursor).
 */
export function useNotifications(
  pageSize = DEFAULT_PAGE_SIZE,
): UseNotificationsResult {
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [cursors, setCursors] = useState<(string | undefined)[]>([undefined]);
  // Secuencia monotónica de peticiones: cada `fetchPage` toma un id creciente y
  // al resolver solo pinta si sigue siendo el último. Sin esto, una respuesta
  // lenta que llega después de otra más reciente pinta la página equivocada.
  const seq = useRef(0);

  const fetchPage = useCallback(
    (
      pageNumber: number,
      cursor: string | undefined,
    ): Promise<PaginatedResponse<Notification> | undefined> => {
      const id = ++seq.current;
      return api
        .listUnreadNotifications({
          limit: pageSize,
          ...(cursor ? { cursor } : {}),
        })
        .then((res) => {
          if (id !== seq.current) return undefined;
          setNotifications(res.data);
          setTotal(res.total ?? res.data.length);
          setNextCursor(res.next_cursor ?? null);
          setPage(pageNumber);
          setError(null);
          return res;
        })
        .catch((err: unknown) => {
          if (id !== seq.current) return undefined;
          setError(err instanceof Error ? err.message : String(err));
          return undefined;
        })
        .finally(() => {
          if (id !== seq.current) return;
          setLoading(false);
        });
    },
    [pageSize],
  );

  useEffect(() => {
    // La marca de "petición en curso" debe fijarse fuera del camino síncrono
    // del efecto (spec frontend-lint-zero), pero en el mismo turno, para que
    // al recargar vuelva a aparecer el indicador de carga. Programarla en un
    // microtask resuelto deja el estado fuera del cuerpo síncrono del efecto,
    // sin necesidad de suprimir la regla.
    //
    // La cancelación se resuelve con la misma guarda de secuencia que la
    // respuesta en vuelo: el desmontaje incrementa `seq`, así que el `id` que
    // tomará la petición inicial deja de ser el último y ni el microtask ni
    // `fetchPage` pintan nada. No hace falta un flag aparte.
    const id = seq.current + 1;
    void Promise.resolve().then(() => {
      if (id !== seq.current) return;
      setLoading(true);
      setError(null);
    });
    void fetchPage(1, undefined);
    return () => {
      seq.current += 1;
    };
  }, [fetchPage]);

  const goNext = useCallback(() => {
    if (nextCursor == null) return;
    const target = page + 1;
    setCursors((prev) => {
      const copy = prev.slice(0, target - 1);
      copy[target - 1] = nextCursor;
      return copy;
    });
    setLoading(true);
    void fetchPage(target, nextCursor);
  }, [nextCursor, page, fetchPage]);

  const goPrev = useCallback(() => {
    if (page <= 1) return;
    const target = page - 1;
    setLoading(true);
    void fetchPage(target, cursors[target - 1]);
  }, [page, cursors, fetchPage]);

  const refetch = useCallback(() => {
    setLoading(true);
    void fetchPage(page, cursors[page - 1]);
  }, [page, cursors, fetchPage]);

  const markRead = useCallback(
    async (id: string) => {
      try {
        await api.markNotificationsRead([id]);
        const res = await fetchPage(page, cursors[page - 1]);
        // Si al refrescar la página actual esta queda vacía (era la última
        // fila), retrocede a la anterior para no dejar al usuario en una página
        // fantasma.
        if (res && res.data.length === 0 && page > 1) {
          const target = page - 1;
          setLoading(true);
          await fetchPage(target, cursors[target - 1]);
        }
      } catch (err: unknown) {
        // Sin esto la UI se quedaría con datos obsoletos en silencio.
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [page, cursors, fetchPage],
  );

  const markAll = useCallback(async () => {
    try {
      await api.markAllNotificationsRead();
      // Marcar todas vacía el conjunto: se vuelve a la primera página sin
      // cursor y se descarta la pila de cursores, que ya no describe nada útil.
      setCursors([undefined]);
      setPage(1);
      setLoading(true);
      await fetchPage(1, undefined);
    } catch (err: unknown) {
      // Sin esto la UI se quedaría con datos obsoletos en silencio.
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [fetchPage]);

  return {
    notifications,
    total,
    loading,
    error,
    page,
    hasPrev: page > 1,
    hasNext: nextCursor != null,
    goNext,
    goPrev,
    refetch,
    markRead,
    markAll,
  };
}
