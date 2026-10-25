import { useState, useCallback, useEffect } from "react";
import type { Note, NoteInput } from "../types";
import { api } from "../api/client";

export interface UseNotesResult {
  notes: Note[];
  loading: boolean;
  /** Fallo de **lectura**: el listado no se pudo cargar. */
  error: string | null;
  /** Crea una nota y recarga el listado. */
  create: (body: NoteInput) => Promise<void>;
  /** Actualiza una nota y recarga el listado. */
  update: (id: string, body: NoteInput) => Promise<void>;
  /** Borra una nota y recarga el listado. */
  remove: (id: string) => Promise<void>;
}

/**
 * Listado de notas: se pide al montar y se recarga tras cada mutación.
 *
 * `create`, `update` y `remove` **rechazan** si la API falla, para que quien
 * edita decida qué hacer con el fallo (avisar y no cerrar el formulario); un
 * fallo de lectura se queda en `error`.
 */
export function useNotes(): UseNotesResult {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchNotes = useCallback(() => {
    return api
      .listNotes()
      .then((data) => {
        setNotes(data);
        setError(null);
      })
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : String(err)),
      )
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    // La marca de "petición en curso" se fija fuera del camino síncrono del
    // efecto (spec frontend-lint-zero), igual que en `useJournal`.
    let cancelled = false;
    void Promise.resolve().then(() => {
      if (cancelled) return;
      setLoading(true);
      setError(null);
    });
    void fetchNotes();
    return () => {
      cancelled = true;
    };
  }, [fetchNotes]);

  const create = useCallback(
    async (body: NoteInput) => {
      await api.createNote(body);
      await fetchNotes();
    },
    [fetchNotes],
  );

  const update = useCallback(
    async (id: string, body: NoteInput) => {
      await api.updateNote(id, body);
      await fetchNotes();
    },
    [fetchNotes],
  );

  const remove = useCallback(
    async (id: string) => {
      await api.deleteNote(id);
      await fetchNotes();
    },
    [fetchNotes],
  );

  return { notes, loading, error, create, update, remove };
}
