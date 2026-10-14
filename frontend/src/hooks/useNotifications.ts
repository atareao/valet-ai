import type { Notification } from "../types";

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

// RED: sin implementar
export function useNotifications(
  pageSize = DEFAULT_PAGE_SIZE,
): UseNotificationsResult {
  void pageSize;
  return {
    notifications: [],
    total: 0,
    loading: true,
    error: null,
    page: 1,
    hasPrev: false,
    hasNext: false,
    goNext() {},
    goPrev() {},
    refetch() {},
    markRead: async () => {},
    markAll: async () => {},
  };
}
