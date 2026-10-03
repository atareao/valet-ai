import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/client";
import type { Tool } from "../types";

export interface UseToolsReturn {
  tools: Tool[];
  loading: boolean;
  error: string | null;
  toggle: (id: string) => Promise<void>;
}

/**
 * Encapsula `GET /api/tools` y `PUT /api/tools/{id}/toggle`. Al montar carga el
 * catálogo; `toggle` reemplaza solo la tool afectada con la que devuelve la API
 * y, en caso de fallo, fija `error` sin re-lanzar (el consumidor reacciona al
 * estado `error`, no a un rechazo).
 */
export function useTools(): UseToolsReturn {
  const [tools, setTools] = useState<Tool[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    api
      .getTools()
      .then((data) => {
        if (!mountedRef.current) return;
        setTools(data);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!mountedRef.current) return;
        setError(
          e instanceof Error ? e.message : "Error al cargar las herramientas",
        );
      })
      .finally(() => {
        if (mountedRef.current) setLoading(false);
      });
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const toggle = useCallback(async (id: string): Promise<void> => {
    // Limpiar al inicio (antes del `await`) para que un fallo posterior con el
    // mismo mensaje vuelva a transicionar null → mensaje y el `useEffect` que
    // observa `error` en ToolsTab vuelva a avisar.
    setError(null);
    try {
      const updated = await api.toggleTool(id);
      setTools((prev) =>
        prev.map((tool) => (tool.id === id ? updated : tool)),
      );
    } catch (e: unknown) {
      setError(
        e instanceof Error ? e.message : "Error al cambiar la herramienta",
      );
    }
  }, []);

  return { tools, loading, error, toggle };
}