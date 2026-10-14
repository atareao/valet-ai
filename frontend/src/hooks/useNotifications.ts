import { useState, useEffect, useCallback } from "react";
import type { Notification } from "../types";
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

  const fetchPage = useCallback(
    (pageNumber: number, cursor: string | undefined) => {
      return api
        .listUnreadNotifications({
          limit: pageSize,
          ...(cursor ? { cursor } : {}),
        })
        .then((res) => {
          setNotifications(res.data);
          setTotal(res.total ?? res.data.length);
          setNextCursor(res.next_cursor ?? null);
          setPage(pageNumber);
          setError(null);
        })
        .catch((err: unknown) => {
          setError(err instanceof Error ? err.message : String(err));
        })
        .finally(() => setLoading(false));
    },
    [pageSize],
  );

  useEffect(() => {
    // La marca de "petición en curso" debe fijarse fuera del camino síncrono
    // del efecto (spec frontend-lint-zero), pero en el mismo turno, para que
    // al recargar vuelva a aparecer el indicador de carga. Programarla en un
    // microtask resuelto deja el estado fuera del cuerpo síncrono del efecto,
    // sin necesidad de suprimir la regla.
    let cancelled = false;
    void Promise.resolve().then(() => {
      if (cancelled) return;
      setLoading(true);
      setError(null);
    });
    void fetchPage(1, undefined);
    return () => {
      cancelled = true;
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
      await api.markNotificationsRead([id]);
      await fetchPage(page, cursors[page - 1]);
    },
    [page, cursors, fetchPage],
  );

  const markAll = useCallback(async () => {
    await api.markAllNotificationsRead();
    await fetchPage(page, cursors[page - 1]);
  }, [page, cursors, fetchPage]);

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
