import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/client";
import type { BlogCheckResult, BlogStatus } from "../types";

export interface UseBlogReturn {
  /** Estado vigente de la configuración; `null` mientras no se ha leído. */
  status: BlogStatus | null;
  /**
   * `true` solo durante la carga inicial de montaje. Las relecturas
   * (`refetch` o el refresco tras `savePassword`) **no** lo vuelven a activar.
   */
  loading: boolean;
  /** Error de la última lectura; `null` si fue bien. */
  error: string | null;
  /** `true` solo mientras dura el guardado de la contraseña; para el botón. */
  saving: boolean;
  /** Relee `GET /api/blog/status` y devuelve la respuesta nueva. */
  refetch: () => Promise<BlogStatus>;
  /**
   * Llama a `PUT /api/blog/password` con la contraseña y refresca el estado. La
   * contraseña **nunca** se guarda por `PUT /api/settings` (es una clave
   * sensible): este endpoint dedicado es su única puerta.
   */
  savePassword: (password: string) => Promise<void>;
  /**
   * Sondea `GET /api/blog/check` y devuelve el diagnóstico. La ruta responde
   * siempre `200`, así que un fallo del Blog **no** rechaza: viaja en
   * `BlogCheckResult.ok`/`detail`. No toca `status` ni `error`: es una
   * comprobación puntual de la instancia remota, no una relectura del estado
   * local de Valet.
   */
  check: () => Promise<BlogCheckResult>;
  /** `true` solo mientras dura una comprobación (`check`); para el botón. */
  checking: boolean;
}

/**
 * Encapsula el estado de la integración con el Blog. Al montar carga
 * `GET /api/blog/status` (nunca la contraseña) y expone `savePassword`, que la
 * persiste por su endpoint dedicado y vuelve a leer el estado. Igual que el
 * resto de hooks de datos, fija `error` sin re-lanzar en la carga y protege el
 * `setState` con `mountedRef` para no actualizar tras el desmontaje.
 */
export function useBlog(): UseBlogReturn {
  const [status, setStatus] = useState<BlogStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<BlogStatus> => {
    return api
      .getBlogStatus()
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
            e instanceof Error ? e.message : "Error al cargar el estado del Blog",
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

  const savePassword = useCallback(
    async (password: string): Promise<void> => {
      if (mountedRef.current) setSaving(true);
      try {
        await api.setBlogPassword(password);
        // Relectura best-effort para reconciliar `configured` con el servidor;
        // su fallo ya se expone por `error`, no como rechazo del guardado.
        await load().catch(() => undefined);
      } finally {
        if (mountedRef.current) setSaving(false);
      }
    },
    [load],
  );

  const check = useCallback(async (): Promise<BlogCheckResult> => {
    if (mountedRef.current) setChecking(true);
    try {
      return await api.checkBlog();
    } finally {
      if (mountedRef.current) setChecking(false);
    }
  }, []);

  return { status, loading, error, saving, refetch: load, savePassword, check, checking };
}
