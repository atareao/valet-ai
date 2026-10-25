import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Se mockea solo el cliente HTTP: el hook `useNotes` y la vista corren de
// verdad (spec `notes-ui`, frontend). `api/client.ts` aún no expone los métodos
// de notas en RED, así que el mock es un objeto propio (hoisted), como en el RED
// de la bitácora. Antd necesita `matchMedia`.
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

import { NotesView } from "./NotesView";

beforeEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

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

/** Las notas de la lista: los `listitem` que muestran su fecha. */
const noteItems = (): HTMLElement[] =>
  screen
    .getAllByRole("listitem")
    .filter((item) => item.querySelector("time") !== null);

/** La nota que está en edición: el `listitem` con el botón «Guardar». */
const itemBeingEdited = (): HTMLElement => {
  const item = screen
    .getAllByRole("listitem")
    .find((li) => within(li).queryByRole("button", { name: /^Guardar$/ }));
  if (!item) throw new Error("no hay ninguna nota en edición");
  return item;
};

describe("NotesView", () => {
  it("lista las notas con su categoría, sus etiquetas y sus fechas", async () => {
    mockApi.listNotes.mockResolvedValue([
      note(),
      note({
        id: "n2",
        content: "otra nota",
        category: "journal",
        tags: "trabajo",
        created_at: "2026-10-16 09:00:00",
      }),
    ]);

    render(<NotesView />);

    expect(await screen.findByText("una idea")).toBeInTheDocument();

    const items = noteItems();
    expect(items).toHaveLength(2);

    const recent = within(items[0]);
    // La categoría se muestra con una etiqueta legible (idea → «Idea»).
    expect(recent.getByText("Idea")).toBeInTheDocument();
    // Las etiquetas viajan como cadena y se muestran separadas.
    expect(recent.getByText("casa")).toBeInTheDocument();
    expect(recent.getByText("ocio")).toBeInTheDocument();
    // La fecha se pinta partiendo la cadena del backend.
    expect(recent.getByText("18 de octubre de 2026")).toBeInTheDocument();

    expect(within(items[1]).getByText("Diario")).toBeInTheDocument();
  });

  it("el formulario crea una nota y recarga el listado", async () => {
    const user = userEvent.setup();
    const created = note({ id: "n9", content: "comprar pan", tags: "casa" });
    mockApi.listNotes
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([created]);
    mockApi.createNote.mockResolvedValue(created);

    render(<NotesView />);

    await user.type(await screen.findByLabelText("Contenido"), "comprar pan");
    await user.click(screen.getByRole("combobox"));
    await user.click(await screen.findByTitle("Diario"));
    await user.type(screen.getByLabelText("Etiquetas"), "casa");
    await user.click(screen.getByRole("button", { name: /^Crear$/ }));

    await waitFor(() => {
      expect(mockApi.createNote).toHaveBeenCalledWith({
        content: "comprar pan",
        category: "journal",
        tags: "casa",
      });
    });
    await waitFor(() => {
      expect(mockApi.listNotes).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText("comprar pan")).toBeInTheDocument();
  });

  it("edita el contenido de una nota existente y recarga el listado", async () => {
    const user = userEvent.setup();
    const edited = note({ content: "contenido editado" });
    mockApi.listNotes
      .mockResolvedValueOnce([note()])
      .mockResolvedValueOnce([edited]);
    mockApi.updateNote.mockResolvedValue(edited);

    render(<NotesView />);

    await user.click(await screen.findByRole("button", { name: /^Editar$/ }));

    const editor = itemBeingEdited();
    const content = within(editor).getByLabelText("Contenido");
    await user.clear(content);
    await user.type(content, "contenido editado");
    await user.click(within(editor).getByRole("button", { name: /^Guardar$/ }));

    await waitFor(() => {
      expect(mockApi.updateNote).toHaveBeenCalledWith(
        "n1",
        expect.objectContaining({ content: "contenido editado" }),
      );
    });
    await waitFor(() => {
      expect(mockApi.listNotes).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText("contenido editado")).toBeInTheDocument();
  });

  it("vaciar las etiquetas al editar las borra enviando tags vacío", async () => {
    const user = userEvent.setup();
    const edited = note({ tags: null });
    mockApi.listNotes
      .mockResolvedValueOnce([note()])
      .mockResolvedValueOnce([edited]);
    mockApi.updateNote.mockResolvedValue(edited);

    render(<NotesView />);

    await user.click(await screen.findByRole("button", { name: /^Editar$/ }));

    const editor = itemBeingEdited();
    const tags = within(editor).getByLabelText("Etiquetas");
    await user.clear(tags);
    await user.click(within(editor).getByRole("button", { name: /^Guardar$/ }));

    await waitFor(() => {
      // `tags: ""` (no `undefined`): el contrato de la API vacía las etiquetas
      // con la cadena vacía, que es un valor y no una ausencia.
      expect(mockApi.updateNote).toHaveBeenCalledWith(
        "n1",
        expect.objectContaining({ tags: "" }),
      );
    });
  });

  it("el borrado pide confirmación antes de llamar a la API", async () => {
    const user = userEvent.setup();
    mockApi.listNotes.mockResolvedValueOnce([note()]).mockResolvedValueOnce([]);
    mockApi.deleteNote.mockResolvedValue(undefined);

    render(<NotesView />);

    await user.click(await screen.findByRole("button", { name: /^Borrar$/ }));

    expect(await screen.findByText(/¿Borrar esta nota\?/)).toBeInTheDocument();
    expect(mockApi.deleteNote).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /^Sí$/ }));

    await waitFor(() => {
      expect(mockApi.deleteNote).toHaveBeenCalledWith("n1");
    });
    expect(
      await screen.findByText(/todavía no hay notas/i),
    ).toBeInTheDocument();
  });

  it("cancelar la confirmación no borra la nota", async () => {
    const user = userEvent.setup();
    mockApi.listNotes.mockResolvedValue([note()]);

    render(<NotesView />);

    await user.click(await screen.findByRole("button", { name: /^Borrar$/ }));
    await user.click(screen.getByRole("button", { name: /^No$/ }));

    expect(mockApi.deleteNote).not.toHaveBeenCalled();
    expect(screen.getByText("una idea")).toBeInTheDocument();
  });

  it("sin notas muestra el estado vacío", async () => {
    mockApi.listNotes.mockResolvedValue([]);

    render(<NotesView />);

    expect(await screen.findByText(/todavía no hay notas/i)).toBeInTheDocument();
  });

  it("el alta sin etiquetas envía tags undefined, no la cadena vacía", async () => {
    const user = userEvent.setup();
    const created = note({ id: "n9", content: "nota sin etiquetas", tags: null });
    mockApi.listNotes.mockResolvedValueOnce([]).mockResolvedValueOnce([created]);
    mockApi.createNote.mockResolvedValue(created);

    render(<NotesView />);

    await user.type(
      await screen.findByLabelText("Contenido"),
      "nota sin etiquetas",
    );
    // La categoría por defecto del alta es «idea», pero el control se resuelve
    // por su etiqueta visible (label → control del `Select`).
    expect(screen.getByLabelText("Categoría")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Crear$/ }));

    await waitFor(() => {
      expect(mockApi.createNote).toHaveBeenCalledWith({
        content: "nota sin etiquetas",
        category: "idea",
        tags: undefined,
      });
    });
    // En el alta vacío es ausencia (`undefined`), no `""`: al contrario que la
    // edición, no se manda una cadena vacía que borraría etiquetas.
    const payload = mockApi.createNote.mock.calls[0][0] as { tags?: string };
    expect(payload.tags).toBeUndefined();
  });

  it("una categoría legada avisa del valor previo y obliga a elegir al guardar", async () => {
    const user = userEvent.setup();
    mockApi.listNotes
      .mockResolvedValueOnce([note({ category: "todo" })])
      .mockResolvedValueOnce([note({ category: "journal" })]);
    mockApi.updateNote.mockResolvedValue(note({ category: "journal" }));

    render(<NotesView />);

    await user.click(await screen.findByRole("button", { name: /^Editar$/ }));

    const editor = itemBeingEdited();
    // El valor previo era legado (`todo`): se muestra junto al editor.
    expect(within(editor).getByText("todo (legado)")).toBeInTheDocument();
    // Sin categoría válida elegida no hay nada que guardar.
    expect(
      within(editor).getByRole("button", { name: /^Guardar$/ }),
    ).toBeDisabled();
    expect(mockApi.updateNote).not.toHaveBeenCalled();

    // Al elegir una de las tres, el guardado se habilita y envía la nueva.
    await user.click(within(editor).getByLabelText("Categoría"));
    await user.click(await screen.findByTitle("Diario"));

    const guardar = within(editor).getByRole("button", { name: /^Guardar$/ });
    expect(guardar).toBeEnabled();
    await user.click(guardar);

    await waitFor(() => {
      expect(mockApi.updateNote).toHaveBeenCalledWith(
        "n1",
        expect.objectContaining({ category: "journal" }),
      );
    });
  });

  it("un fallo al editar avisa y deja el editor abierto", async () => {
    const user = userEvent.setup();
    mockApi.listNotes.mockResolvedValue([note()]);
    mockApi.updateNote.mockRejectedValue(new Error("boom"));

    render(<NotesView />);

    await user.click(await screen.findByRole("button", { name: /^Editar$/ }));

    const editor = itemBeingEdited();
    const content = within(editor).getByLabelText("Contenido");
    await user.clear(content);
    await user.type(content, "contenido editado");
    await user.click(within(editor).getByRole("button", { name: /^Guardar$/ }));

    expect(await screen.findByText("boom")).toBeInTheDocument();
    // El editor no se cierra y conserva lo escrito para reintentar. El
    // `aria-label` del botón se ensucia un instante con la animación del
    // indicador de carga de antd, así que se compara el nombre sin anclas.
    const stillEditing = screen.getAllByRole("listitem")[0];
    expect(within(stillEditing).getByLabelText("Contenido")).toHaveValue(
      "contenido editado",
    );
    expect(
      within(stillEditing).getByRole("button", { name: /Guardar/ }),
    ).toBeInTheDocument();
    // No hay recarga del listado: el fallo no la dispara.
    expect(mockApi.listNotes).toHaveBeenCalledTimes(1);
  });

  it("un fallo al crear avisa y conserva el borrador", async () => {
    const user = userEvent.setup();
    mockApi.listNotes.mockResolvedValue([]);
    mockApi.createNote.mockRejectedValue(new Error("boom"));

    render(<NotesView />);

    await user.type(await screen.findByLabelText("Contenido"), "comprar pan");
    await user.click(screen.getByRole("button", { name: /^Crear$/ }));

    expect(await screen.findByText("boom")).toBeInTheDocument();
    // El borrador sigue abierto y conserva el contenido.
    expect(screen.getByLabelText("Contenido")).toHaveValue("comprar pan");
  });

  it("muestra un estado de carga mientras la petición está en curso", () => {
    mockApi.listNotes.mockReturnValue(new Promise(() => {}));

    render(<NotesView />);

    expect(screen.getByRole("status")).toHaveTextContent(/cargando las notas/i);
    expect(screen.queryByText("una idea")).not.toBeInTheDocument();
  });

  it("muestra un estado de error si la petición falla", async () => {
    mockApi.listNotes.mockRejectedValue(new Error("no notes"));

    render(<NotesView />);

    expect(
      await screen.findByText("No se pudieron cargar las notas"),
    ).toBeInTheDocument();
    expect(screen.getByText("no notes")).toBeInTheDocument();
  });
});
