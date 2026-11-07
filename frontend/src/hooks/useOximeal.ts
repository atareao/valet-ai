import { useState, useEffect, useCallback, useRef } from "react";
import { checkOximeal, getOximealStatus, setOximealSecret } from "../api/oximeal";
import type { OximealCheckResult, OximealStatus } from "../types";

export interface UseOximealReturn {
  /** Estado vigente de la configuración; `null` mientras no se ha leído. */
  status: OximealStatus | null;
  /**
   * `true` solo durante la carga inicial de montaje. Las relecturas
   * (`refetch` o el refresco tras `saveSecret`) **no** lo vuelven a activar.
   */
  loading: boolean;
  /** Error de la última lectura; `null` si fue bien. Estado degradado. */
  error: string | null;
  /** `true` solo mientras dura el guardado del secreto; para el botón. */
  saving: boolean;
  /** Relee `GET /api/oximeal/status` y devuelve la respuesta nueva. */
  refetch: () => Promise<OximealStatus>;
  /**
   * Llama a `PUT /api/oximeal/secret` con el `client_secret` y refresca el
   * estado. El secreto **nunca** se guarda por `PUT /api/settings` (es una clave
   * sensible): este endpoint dedicado es su única puerta y su valor no se
   * vuelve a leer.
   */
  saveSecret: (secret: string) => Promise<void>;
  /**
   * Sondea `GET /api/oximeal/check` y devuelve el diagnóstico. La ruta responde
   * siempre `200`, así que un fallo de OxiMeal **no** rechaza: viaja en
   * `OximealCheckResult.ok`/`detail`. No toca `status` ni `error`: es una
   * comprobación puntual de la instancia remota, no una relectura del estado
   * local de Valet.
   */
  check: () => Promise<OximealCheckResult>;
  /** `true` solo mientras dura una comprobación (`check`); para el botón. */
  checking: boolean;
}

/**
 * Encapsula el estado de la integración con OxiMeal. Al montar carga
 * `GET /api/oximeal/status` (nunca el `client_secret`) y expone `saveSecret`,
 * que lo persiste por su endpoint dedicado y vuelve a leer el estado. Igual que
 * el resto de hooks de datos, fija `error` sin re-lanzar en la carga y protege
 * el `setState` con `mountedRef` para no actualizar tras el desmontaje.
 */
export function useOximeal(): UseOximealReturn {
  const [status, setStatus] = useState<OximealStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<OximealStatus> => {
    return getOximealStatus()
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
              : "Error al cargar el estado de OxiMeal",
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

  const saveSecret = useCallback(
    async (secret: string): Promise<void> => {
      if (mountedRef.current) setSaving(true);
      try {
        await setOximealSecret(secret);
        // Relectura best-effort para reconciliar `configured` con el servidor;
        // su fallo ya se expone por `error`, no como rechazo del guardado.
        await load().catch(() => undefined);
      } finally {
        if (mountedRef.current) setSaving(false);
      }
    },
    [load],
  );

  const check = useCallback(async (): Promise<OximealCheckResult> => {
    if (mountedRef.current) setChecking(true);
    try {
      return await checkOximeal();
    } finally {
      if (mountedRef.current) setChecking(false);
    }
  }, []);

  return { status, loading, error, saving, refetch: load, saveSecret, check, checking };
}
