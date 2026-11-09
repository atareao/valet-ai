import { useState, useEffect, useCallback, useRef } from "react";
import {
  checkGithub,
  getGithubStatus,
  putGithubToken,
} from "../api/github";
import type { GithubCheckResult, GithubStatus } from "../types";

export interface UseGithubReturn {
  /** Estado vigente de la configuración; `null` mientras no se ha leído. */
  status: GithubStatus | null;
  /**
   * `true` solo durante la carga inicial de montaje. Las relecturas
   * (`saveToken`, que refresca el estado) **no** lo vuelven a activar.
   */
  loading: boolean;
  /** Error de la última lectura; `null` si fue bien. */
  error: string | null;
  /** `true` solo mientras dura el guardado del token; para el botón. */
  saving: boolean;
  /**
   * Llama a `PUT /api/github/token` con el token y refresca el estado. El token
   * **nunca** se guarda por `PUT /api/settings` (es una clave sensible): este
   * endpoint dedicado es su única puerta y su valor no se vuelve a leer.
   */
  saveToken: (token: string) => Promise<void>;
  /**
   * Sondea `GET /api/github/check` y devuelve el diagnóstico. La ruta responde
   * siempre `200`, así que un fallo de GitHub **no** rechaza: viaja en
   * `GithubCheckResult.ok`/`detail`. No toca `status` ni `error`: es una
   * comprobación puntual de la API remota, no una relectura del estado local de
   * Valet.
   */
  check: () => Promise<GithubCheckResult>;
  /** `true` solo mientras dura una comprobación (`check`); para el botón. */
  checking: boolean;
}

/**
 * Encapsula el estado de la integración con GitHub. Al montar carga
 * `GET /api/github/status` (nunca el token) y expone `saveToken`, que lo
 * persiste por su endpoint dedicado y vuelve a leer el estado. Igual que el
 * resto de hooks de datos, fija `error` sin re-lanzar en la carga y protege el
 * `setState` con `mountedRef` para no actualizar tras el desmontaje.
 */
export function useGithub(): UseGithubReturn {
  const [status, setStatus] = useState<GithubStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<GithubStatus> => {
    return getGithubStatus()
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
              : "Error al cargar el estado de GitHub",
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
        await putGithubToken(token);
        // Relectura best-effort para reconciliar `configured` con el servidor;
        // su fallo ya se expone por `error`, no como rechazo del guardado.
        await load().catch(() => undefined);
      } finally {
        if (mountedRef.current) setSaving(false);
      }
    },
    [load],
  );

  const check = useCallback(async (): Promise<GithubCheckResult> => {
    if (mountedRef.current) setChecking(true);
    try {
      return await checkGithub();
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
