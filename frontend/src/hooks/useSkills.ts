import { useState, useEffect, useRef } from "react";
import { api } from "../api/client";
import type { SkillInfo } from "../types";

export interface UseSkillsReturn {
  skills: SkillInfo[];
  coreTools: string[];
  loading: boolean;
  error: string | null;
}

/**
 * Encapsula `GET /api/skills`: al montar carga el catálogo cerrado de skills
 * enrutables y el conjunto de herramientas núcleo. Igual que `useTools`, fija
 * `error` sin re-lanzar y protege el `setState` con `mountedRef` para no
 * actualizar tras el desmontaje. Si la llamada falla, el consumidor decide el
 * fallback (la pestaña de prompts cae a las claves de settings).
 */
export function useSkills(): UseSkillsReturn {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [coreTools, setCoreTools] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    api
      .getSkills()
      .then((data) => {
        if (!mountedRef.current) return;
        setSkills(data.skills);
        setCoreTools(data.core_tools);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!mountedRef.current) return;
        setError(
          e instanceof Error ? e.message : "Error al cargar las skills",
        );
      })
      .finally(() => {
        if (mountedRef.current) setLoading(false);
      });
    return () => {
      mountedRef.current = false;
    };
  }, []);

  return { skills, coreTools, loading, error };
}
