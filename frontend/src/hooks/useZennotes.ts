import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/client";
import type { ZennotesStatus } from "../types";

export interface UseZennotesReturn {
  /** Estado vigente de la configuración; `null` mientras no se ha leído. */
  status: ZennotesStatus | null;
  /**
   * `true` solo durante la carga inicial de montaje. Las relecturas
   * (`refetch` o el refresco tras `saveToken`) **no** lo vuelven a activar.
   */
  loading: boolean;
  /** Error de la última lectura; `null` si fue bien. */
  error: string | null;
  /** `true` solo mientras dura el guardado del token; para el botón. */
  saving: boolean;
  /** Relee `GET /api/zennotes/status` y devuelve la respuesta nueva. */
  refetch: () => Promise<ZennotesStatus>;
  /**
   * Llama a `PUT /api/zennotes/token` con el token y refresca el estado. El
   * token **nunca** se guarda por `PUT /api/settings` (es una clave sensible):
   * este endpoint dedicado es su única puerta.
   */
  saveToken: (token: string) => Promise<void>;
}

/**
 * Encapsula el estado de la integración con ZenNotes. Al montar carga
 * `GET /api/zennotes/status` (nunca el token) y expone `saveToken`, que lo
 * persiste por su endpoint dedicado y vuelve a leer el estado. Igual que el
 * resto de hooks de datos, fija `error` sin re-lanzar en la carga y protege el
 * `setState` con `mountedRef` para no actualizar tras el desmontaje.
 */
export function useZennotes(): UseZennotesReturn {
  const [status, setStatus] = useState<ZennotesStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<ZennotesStatus> => {
    return api
      .getZennotesStatus()
      .then((data) => {
        if (mountedRef.current) {
          setStatus(data);
          setError(null);
        }
        return data;
      })
      .catch((e: unknown) => {
        if (mountedRef.current) {
          setError(
            e instanceof Error
              ? e.message
              : "Error al cargar el estado de ZenNotes",
          );
        }
        throw e;
      })
      .finally(() => {
        if (mountedRef.current) setLoading(false);
      });
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    // El error queda en el estado; el consumidor decide el fallback.
    load().catch(() => undefined);
    return () => {
      mountedRef.current = false;
    };
  }, [load]);

  const saveToken = useCallback(
    async (token: string): Promise<void> => {
      if (mountedRef.current) setSaving(true);
      try {
        await api.setZennotesToken(token);
        // Relectura best-effort para reconciliar `configured` con el servidor;
        // su fallo ya se expone por `error`, no como rechazo del guardado.
        await load().catch(() => undefined);
      } finally {
        if (mountedRef.current) setSaving(false);
      }
    },
    [load],
  );

  return { status, loading, error, saving, refetch: load, saveToken };
}
