import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/client";
import type { StravaStatus } from "../types";

export interface UseStravaReturn {
  /** Estado vigente de la conexión; `null` mientras no se ha leído todavía. */
  status: StravaStatus | null;
  /**
   * `true` solo durante la carga inicial de montaje. Las relecturas
   * (`refetch` o el refresco tras `disconnect`) **no** lo vuelven a activar.
   */
  loading: boolean;
  /** Error de la última lectura; `null` si fue bien. */
  error: string | null;
  /** Relee `GET /api/strava/status` y devuelve la respuesta nueva. */
  refetch: () => Promise<StravaStatus>;
  /**
   * Llama a `POST /api/strava/disconnect` y deja el estado en «no conectada».
   *
   * Rechaza **solo** si la propia desconexión falla: el consumidor usa ese
   * rechazo para mostrar «Error al desconectar». El refresco posterior del
   * estado es best-effort; si falla, el error se expone por `error` (aviso de
   * estado), nunca como fallo de la desconexión.
   */
  disconnect: () => Promise<void>;
}

/**
 * Encapsula el estado de la conexión con Strava. Al montar carga
 * `GET /api/strava/status` (nunca tokens) y expone `disconnect`, que revoca en
 * el backend y vuelve a leer el estado. Igual que el resto de hooks de datos,
 * fija `error` sin re-lanzar y protege el `setState` con `mountedRef` para no
 * actualizar tras el desmontaje.
 */
export function useStrava(): UseStravaReturn {
  const [status, setStatus] = useState<StravaStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<StravaStatus> => {
    return api
      .getStravaStatus()
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
            e instanceof Error ? e.message : "Error al cargar el estado de Strava",
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

  const disconnect = useCallback(async () => {
    // Solo el fallo de la propia desconexión debe rechazar.
    await api.disconnectStrava();
    // El servidor ya confirmó la desconexión: reflejarla de inmediato para no
    // seguir mostrando «Conectada» si la relectura posterior fallara.
    if (mountedRef.current) {
      setStatus({
        connected: false,
        athlete_id: null,
        athlete_name: null,
        scope: null,
      });
    }
    // Relectura best-effort para reconciliar con el servidor. Su fallo se
    // expone por `error` (aviso aparte), no como rechazo de `disconnect`.
    await load().catch(() => undefined);
  }, [load]);

  return { status, loading, error, refetch: load, disconnect };
}
