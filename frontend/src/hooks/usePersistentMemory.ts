import { useState, useEffect, useCallback, useRef } from "react";
import { api, ApiError } from "../api/client";
import type { PersistentMemoryState } from "../types";

const ALLOWED_TOP_LEVEL_KEYS = ["schema_version", "user_profile", "system_rules"];

export type PersistentMemorySaveOutcome =
  | "saved"
  | "invalid"
  | "conflict"
  | "error";

export interface UsePersistentMemoryReturn {
  state: PersistentMemoryState | null;
  loading: boolean;
  saving: boolean;
  error: string | null;
  warning: string | null;
  conflict: string | null;
  save: (payloadText: string) => Promise<PersistentMemorySaveOutcome>;
  clear: () => Promise<boolean>;
}

/**
 * Valida en cliente la forma del payload antes de enviarlo. El servidor sigue
 * siendo la autoridad; esto solo evita viajes evidentemente inválidos.
 * Devuelve un mensaje de error o `null` si el payload es aceptable.
 */
export function validatePersistentPayload(parsed: unknown): string | null {
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return "El JSON debe ser un objeto";
  }
  const obj = parsed as Record<string, unknown>;

  if (obj.schema_version !== 1) {
    return "schema_version debe ser 1";
  }

  const unknownKey = Object.keys(obj).find(
    (key) => !ALLOWED_TOP_LEVEL_KEYS.includes(key),
  );
  if (unknownKey) {
    return `Clave no permitida: ${unknownKey}`;
  }

  if (
    obj.user_profile !== undefined &&
    (obj.user_profile === null ||
      typeof obj.user_profile !== "object" ||
      Array.isArray(obj.user_profile))
  ) {
    return "user_profile debe ser un objeto";
  }

  if (obj.system_rules !== undefined) {
    if (!Array.isArray(obj.system_rules)) {
      return "system_rules debe ser un array";
    }
    if (!obj.system_rules.every((rule) => typeof rule === "string")) {
      return "system_rules debe contener solo strings";
    }
  }

  return null;
}

export function usePersistentMemory(): UsePersistentMemoryReturn {
  const [state, setState] = useState<PersistentMemoryState | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [warning, setWarning] = useState<string | null>(null);
  const [conflict, setConflict] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback(() => {
    return api
      .getPersistentMemory()
      .then((data) => {
        if (!mountedRef.current) return;
        setState(data);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!mountedRef.current) return;
        setError(
          e instanceof Error
            ? e.message
            : "Error al cargar la memoria persistente",
        );
      })
      .finally(() => {
        if (mountedRef.current) setLoading(false);
      });
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void load();
    return () => {
      mountedRef.current = false;
    };
  }, [load]);

  const save = useCallback(
    async (payloadText: string): Promise<PersistentMemorySaveOutcome> => {
      setSaving(true);
      setError(null);
      setWarning(null);
      setConflict(null);

      let parsed: unknown;
      try {
        parsed = JSON.parse(payloadText);
      } catch {
        setError("El texto no es JSON válido");
        setSaving(false);
        return "invalid";
      }

      const validationError = validatePersistentPayload(parsed);
      if (validationError) {
        setError(validationError);
        setSaving(false);
        return "invalid";
      }

      try {
        const result = await api.updatePersistentMemory(
          parsed as Record<string, unknown>,
          state?.updated_at ?? null,
        );
        setState(result);
        setWarning(result.warning ?? null);
        return "saved";
      } catch (e: unknown) {
        if (e instanceof ApiError && e.status === 409) {
          setConflict(
            "El estado cambió mientras editabas. Se ha recargado el estado vigente.",
          );
          setLoading(true);
          await load();
          return "conflict";
        }
        setError(
          e instanceof Error
            ? e.message
            : "Error al guardar la memoria persistente",
        );
        return "error";
      } finally {
        setSaving(false);
      }
    },
    [load, state],
  );

  const clear = useCallback(async (): Promise<boolean> => {
    setSaving(true);
    setError(null);
    setWarning(null);
    setConflict(null);
    try {
      await api.clearPersistentMemory();
      setLoading(true);
      await load();
      return true;
    } catch (e: unknown) {
      setError(
        e instanceof Error
          ? e.message
          : "Error al vaciar la memoria persistente",
      );
      return false;
    } finally {
      setSaving(false);
    }
  }, [load]);

  return { state, loading, saving, error, warning, conflict, save, clear };
}
