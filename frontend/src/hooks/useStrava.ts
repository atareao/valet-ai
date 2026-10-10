import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/client";
import type { StravaCheckResult, StravaStatus } from "../types";

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
   * Sondea `GET /api/strava/check` y devuelve el diagnóstico. La ruta responde
   * siempre `200`, así que un fallo de Strava **no** rechaza: viaja en
   * `StravaCheckResult.ok`/`error`. No toca `status` ni `error`: es una
   * comprobación puntual, no una relectura del estado.
   */
  check: () => Promise<StravaCheckResult>;
  /** `true` solo mientras dura una comprobación (`check`); para el botón. */
  checking: boolean;
  /**
   * Llama a `POST /api/strava/disconnect` y deja el estado en «no conectada».
   *
   * Rechaza **solo** si la propia desconexión falla: el consumidor usa ese
   * rechazo para mostrar «Error al desconectar». Cuando resuelve, lleva el
   * aviso del servidor (`warning`) o `null`: si la revocación remota no se pudo
   * confirmar, el usuario debe retirar el acceso a mano. El refresco posterior
   * del estado es best-effort; si falla, el error se expone por `error` (aviso
   * de estado), nunca como fallo de la desconexión.
   */
  disconnect: () => Promise<{ warning: string | null }>;
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
  const [checking, setChecking] = useState(false);
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

  const check = useCallback(async (): Promise<StravaCheckResult> => {
    if (mountedRef.current) setChecking(true);
    try {
      return await api.checkStrava();
    } finally {
      if (mountedRef.current) setChecking(false);
    }
  }, []);

  const disconnect = useCallback(async (): Promise<{ warning: string | null }> => {
    // Solo el fallo de la propia desconexión debe rechazar.
    const result = await api.disconnectStrava();
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
    return { warning: result.warning ?? null };
  }, [load]);

  return { status, loading, checking, error, refetch: load, check, disconnect };
}
