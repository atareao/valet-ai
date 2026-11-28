import { useState, useEffect, useCallback, useRef } from "react";
import {
  checkMastodon,
  getMastodonStatus,
  putMastodonToken,
} from "../api/mastodon";
import type { MastodonCheckResult, MastodonStatus } from "../types";

export interface UseMastodonReturn {
  /** Estado vigente de la configuración; `null` mientras no se ha leído. */
  status: MastodonStatus | null;
  /** `true` solo durante la carga inicial; las relecturas no lo reactivan. */
  loading: boolean;
  /** Error de la última lectura; `null` si fue bien. */
  error: string | null;
  /** `true` solo mientras dura el guardado del token; para el botón. */
  saving: boolean;
  /**
   * Llama a `PUT /api/mastodon/token` y refresca el estado. El token **nunca**
   * se guarda por `PUT /api/settings` (es sensible): este endpoint dedicado es
   * su única puerta y su valor no se vuelve a leer.
   */
  saveToken: (token: string) => Promise<void>;
  /**
   * Sondea `GET /api/mastodon/check` y devuelve el diagnóstico. La ruta responde
   * siempre `200`, así que un fallo de Mastodon **no** rechaza: viaja en
   * `MastodonCheckResult.ok`/`detail`. No toca `status` ni `error`.
   */
  check: () => Promise<MastodonCheckResult>;
  /** `true` solo mientras dura una comprobación; para el botón. */
  checking: boolean;
}

/**
 * Encapsula el estado de la integración con Mastodon. Al montar carga
 * `GET /api/mastodon/status` (nunca el token) y expone `saveToken`, que lo
 * persiste por su endpoint dedicado y vuelve a leer el estado. Igual que el
 * resto de hooks de datos, fija `error` sin re-lanzar en la carga y protege el
 * `setState` con `mountedRef` para no actualizar tras el desmontaje.
 */
export function useMastodon(): UseMastodonReturn {
  const [status, setStatus] = useState<MastodonStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<MastodonStatus> => {
    return getMastodonStatus()
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
              : "Error al cargar el estado de Mastodon",
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
        await putMastodonToken(token);
        // Relectura best-effort para reconciliar `configured` con el servidor;
        // su fallo ya se expone por `error`, no como rechazo del guardado.
        await load().catch(() => undefined);
      } finally {
        if (mountedRef.current) setSaving(false);
      }
    },
    [load],
  );

  const check = useCallback(async (): Promise<MastodonCheckResult> => {
    if (mountedRef.current) setChecking(true);
    try {
      return await checkMastodon();
    } finally {
      if (mountedRef.current) setChecking(false);
    }
  }, []);

  return {
    status,
    loading,
    error,
    saving,
    saveToken,
    check,
    checking,
  };
}
