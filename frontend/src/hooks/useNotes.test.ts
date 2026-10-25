import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

// Se mockea solo el cliente HTTP: el hook `useNotes` corre de verdad (spec
// `notes-ui`, frontend). `api/client.ts` aún no expone los métodos de notas en
// RED, así que el mock es un objeto propio (hoisted), igual que en el RED de la
// bitácora.
const mockApi = vi.hoisted(() => ({
  listNotes: vi.fn(),
  createNote: vi.fn(),
  updateNote: vi.fn(),
  deleteNote: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: mockApi,
  BASE_URL: "/api",
}));

import { useNotes } from "./useNotes";

/**
 * Espejo local del `Note` del contrato (tarea 5.1 lo añadirá a `src/types`).
 * Se declara aquí para que el RED compile sin tocar `types`.
 */
interface Note {
  id: string;
  profile_id: string;
  content: string;
  category: string;
  tags: string | null;
  created_at: string;
  updated_at: string;
}

const note = (overrides: Partial<Note> = {}): Note => ({
  id: "n1",
  profile_id: "profile-id",
  content: "una idea",
  category: "idea",
  tags: "casa, ocio",
  created_at: "2026-10-18 11:02:05",
  updated_at: "2026-10-18 11:02:05",
  ...overrides,
});

describe("useNotes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("carga el listado de notas al montar", async () => {
    mockApi.listNotes.mockResolvedValue([
      note(),
      note({ id: "n2", content: "otra nota" }),
    ]);

    const { result } = renderHook(() => useNotes());

    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(mockApi.listNotes).toHaveBeenCalledTimes(1);
    expect(result.current.notes).toHaveLength(2);
    expect(result.current.error).toBeNull();
  });

  it("create llama a la API y recarga el listado", async () => {
    mockApi.listNotes.mockResolvedValue([note()]);
    mockApi.createNote.mockResolvedValue(note({ id: "n3", content: "nueva" }));

    const { result } = renderHook(() => useNotes());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.create({
        content: "nueva",
        category: "idea",
        tags: "x",
      });
    });

    expect(mockApi.createNote).toHaveBeenCalledWith({
      content: "nueva",
      category: "idea",
      tags: "x",
    });
    // Tras mutar, el listado se recarga.
    expect(mockApi.listNotes).toHaveBeenCalledTimes(2);
  });

  it("update llama a la API y recarga el listado", async () => {
    mockApi.listNotes.mockResolvedValue([note()]);
    mockApi.updateNote.mockResolvedValue(note({ content: "editada" }));

    const { result } = renderHook(() => useNotes());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.update("n1", {
        content: "editada",
        category: "idea",
        tags: "casa",
      });
    });

    expect(mockApi.updateNote).toHaveBeenCalledWith("n1", {
      content: "editada",
      category: "idea",
      tags: "casa",
    });
    expect(mockApi.listNotes).toHaveBeenCalledTimes(2);
  });

  it("remove llama a la API y recarga el listado", async () => {
    mockApi.listNotes.mockResolvedValue([note()]);
    mockApi.deleteNote.mockResolvedValue(undefined);

    const { result } = renderHook(() => useNotes());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.remove("n1");
    });

    expect(mockApi.deleteNote).toHaveBeenCalledWith("n1");
    expect(mockApi.listNotes).toHaveBeenCalledTimes(2);
  });

  it("create rechaza si la API falla", async () => {
    mockApi.listNotes.mockResolvedValue([]);
    mockApi.createNote.mockRejectedValue(new Error("content must not be empty"));

    const { result } = renderHook(() => useNotes());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await expect(
      result.current.create({ content: "x", category: "idea" }),
    ).rejects.toThrow("content must not be empty");
  });

  it("update rechaza si la API falla", async () => {
    mockApi.listNotes.mockResolvedValue([]);
    mockApi.updateNote.mockRejectedValue(new Error("note not found"));

    const { result } = renderHook(() => useNotes());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await expect(
      result.current.update("n1", { content: "x", category: "idea" }),
    ).rejects.toThrow("note not found");
  });

  it("remove rechaza si la API falla", async () => {
    mockApi.listNotes.mockResolvedValue([]);
    mockApi.deleteNote.mockRejectedValue(new Error("note not found"));

    const { result } = renderHook(() => useNotes());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await expect(result.current.remove("n1")).rejects.toThrow("note not found");
  });
});
