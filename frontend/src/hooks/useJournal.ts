import { useState, useCallback, useEffect } from "react";
import type { JournalEntry, JournalEntryInput } from "../types";
import { api } from "../api/client";

export interface UseJournalResult {
  entries: JournalEntry[];
  loading: boolean;
  /** Fallo de **lectura**: el listado no se pudo cargar. */
  error: string | null;
  /** Crea o reemplaza la entrada del día y recarga la lista. */
  save: (date: string, body: JournalEntryInput) => Promise<void>;
  /** Borra la entrada del día y recarga la lista. */
  remove: (date: string) => Promise<void>;
}

/**
 * Listado de la bitácora: se pide al montar y se recarga tras cada mutación.
 *
 * `save` y `remove` **rechazan** si la API falla, para que quien edita decida
 * qué hacer con el fallo (avisar y no cerrar el formulario); un fallo de
 * lectura se queda en `error`.
 */
export function useJournal(): UseJournalResult {
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchEntries = useCallback(() => {
    return api
      .listJournalEntries()
      .then((data) => {
        setEntries(data);
        setError(null);
      })
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : String(err)),
      )
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    // La marca de "petición en curso" debe fijarse fuera del camino síncrono
    // del efecto (spec frontend-lint-zero), igual que en `useEvents`.
    let cancelled = false;
    void Promise.resolve().then(() => {
      if (cancelled) return;
      setLoading(true);
      setError(null);
    });
    void fetchEntries();
    return () => {
      cancelled = true;
    };
  }, [fetchEntries]);

  const save = useCallback(
    async (date: string, body: JournalEntryInput) => {
      await api.upsertJournalEntry(date, body);
      await fetchEntries();
    },
    [fetchEntries],
  );

  const remove = useCallback(
    async (date: string) => {
      await api.deleteJournalEntry(date);
      await fetchEntries();
    },
    [fetchEntries],
  );

  return { entries, loading, error, save, remove };
}
