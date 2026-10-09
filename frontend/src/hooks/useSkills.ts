import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/client";
import type { SkillInfo, SkillsResponse } from "../types";

export interface UseSkillsReturn {
  skills: SkillInfo[];
  coreTools: string[];
  loading: boolean;
  error: string | null;
  /**
   * Relee `GET /api/skills` y devuelve la respuesta nueva (rechaza si la
   * llamada falla). La usa la acción de restaurar un campo sobrescrito.
   */
  refetch: () => Promise<SkillsResponse>;
}

/**
 * Encapsula `GET /api/skills`: al montar carga el catálogo cerrado de skills
 * enrutables y el conjunto de herramientas núcleo. Igual que `useTools`, fija
 * `error` sin re-lanzar y protege el `setState` con `mountedRef` para no
 * actualizar tras el desmontaje. Si la llamada falla, el consumidor decide el
 * fallback (la pestaña de prompts degrada sin romper el formulario).
 */
export function useSkills(): UseSkillsReturn {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [coreTools, setCoreTools] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback((): Promise<SkillsResponse> => {
    return api
      .getSkills()
      .then((data) => {
        if (mountedRef.current) {
          setSkills(data.skills);
          setCoreTools(data.core_tools);
          setError(null);
        }
        return data;
      })
      .catch((e: unknown) => {
        if (mountedRef.current) {
          setError(e instanceof Error ? e.message : "Error al cargar las skills");
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

  return { skills, coreTools, loading, error, refetch: load };
}
