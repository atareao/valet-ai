import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { JournalEntry } from "../types";

// Se mockea solo el cliente HTTP: el hook `useJournal` corre de verdad
// (spec `journal-entries`, frontend). Antd necesita `matchMedia`.
// `api/client.ts` aún no expone los métodos de la bitácora en RED, así que el
// mock es un objeto propio (hoisted).
const mockApi = vi.hoisted(() => ({
  listJournalEntries: vi.fn(),
  getJournalEntry: vi.fn(),
  upsertJournalEntry: vi.fn(),
  deleteJournalEntry: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: mockApi,
  BASE_URL: "/api",
}));

import { JournalView } from "./JournalView";

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

const entry = (overrides: Partial<JournalEntry> = {}): JournalEntry => ({
  id: "c2f4aca4-97e0-4dd0-9eda-94f083608b4a",
  profile_id: "profile-id",
  date: "2026-10-18",
  mood: "good",
  content: "un día largo",
  achievements: ["terminar el informe"],
  struggles: ["dormir poco"],
  tags: ["trabajo"],
  created_at: "2026-10-18 11:02:05",
  updated_at: "2026-10-18 11:02:05",
  ...overrides,
});

/** Las entradas de la lista: los `listitem` que muestran su fecha. */
const entryItems = () =>
  screen
    .getAllByRole("listitem")
    .filter((item) => item.querySelector("time") !== null);

describe("JournalView", () => {
  it("lista las entradas de más reciente a más antigua con fecha, ánimo, relato, etiquetas, logros y dificultades", async () => {
    mockApi.listJournalEntries.mockResolvedValue([
      entry(),
      entry({
        id: "e2",
        date: "2026-10-16",
        mood: "terrible",
        content: "otro día",
        achievements: ["llamar a mamá"],
        tags: ["casa"],
      }),
    ]);

    render(<JournalView />);

    expect(await screen.findByText("un día largo")).toBeInTheDocument();

    const items = entryItems();
    expect(items).toHaveLength(2);
    // El backend ya entrega de más reciente a más antigua: el orden se respeta.
    expect(within(items[0]).getByText("18 de octubre de 2026")).toBeInTheDocument();
    expect(within(items[1]).getByText("16 de octubre de 2026")).toBeInTheDocument();

    const recent = within(items[0]);
    expect(recent.getByText("🙂 Bien")).toBeInTheDocument();
    expect(recent.getByText("Etiquetas:")).toBeInTheDocument();
    expect(recent.getByText("trabajo")).toBeInTheDocument();
    expect(recent.getByText("Logros:")).toBeInTheDocument();
    expect(recent.getByText("terminar el informe")).toBeInTheDocument();
    expect(recent.getByText("Dificultades:")).toBeInTheDocument();
    expect(recent.getByText("dormir poco")).toBeInTheDocument();
  });

  it("muestra la fecha del backend tal cual, sin moverla de día", async () => {
    mockApi.listJournalEntries.mockResolvedValue([entry({ date: "2026-01-01" })]);

    render(<JournalView />);

    // Con `new Date("2026-01-01")` en una zona al oeste de UTC esto pintaría el
    // 31 de diciembre: la fecha se parte, no se reinterpreta.
    const time = await screen.findByText("1 de enero de 2026");
    expect(time.tagName).toBe("TIME");
    expect(time).toHaveAttribute("dateTime", "2026-01-01");
  });

  it("una entrada sin ánimo, sin etiquetas y sin logros se muestra sin bloques vacíos", async () => {
    mockApi.listJournalEntries.mockResolvedValue([
      entry({ mood: null, tags: [], achievements: [], struggles: [] }),
    ]);

    render(<JournalView />);

    expect(await screen.findByText("Sin ánimo")).toBeInTheDocument();
    expect(screen.getByText("un día largo")).toBeInTheDocument();
    expect(screen.queryByText("Etiquetas:")).not.toBeInTheDocument();
    expect(screen.queryByText("Logros:")).not.toBeInTheDocument();
    expect(screen.queryByText("Dificultades:")).not.toBeInTheDocument();
  });

  it("editar el ánimo, el relato y las etiquetas llama a la API, conserva lo no tocado y refresca", async () => {
    const user = userEvent.setup();
    const edited = entry({
      mood: "great",
      content: "un día distinto",
      tags: ["trabajo", "casa"],
    });
    mockApi.listJournalEntries
      .mockResolvedValueOnce([entry()])
      .mockResolvedValueOnce([edited]);
    mockApi.upsertJournalEntry.mockResolvedValue(edited);

    render(<JournalView />);

    await user.click(await screen.findByRole("button", { name: /^Editar$/ }));

    await user.click(screen.getByRole("radio", { name: /^Muy bien$/ }));

    const content = screen.getByLabelText("Relato");
    await user.clear(content);
    await user.type(content, "un día distinto");

    const tags = screen.getByLabelText("Etiquetas");
    await user.clear(tags);
    await user.type(tags, "trabajo, casa");

    await user.click(screen.getByRole("button", { name: /^Guardar$/ }));

    await waitFor(() => {
      expect(mockApi.upsertJournalEntry).toHaveBeenCalledWith("2026-10-18", {
        content: "un día distinto",
        mood: "great",
        tags: ["trabajo", "casa"],
        // `PUT` reemplaza la entrada entera: los logros y las dificultades, que
        // el formulario no edita, viajan tal cual para no perderlos.
        achievements: ["terminar el informe"],
        struggles: ["dormir poco"],
      });
    });
    expect(await screen.findByText("un día distinto")).toBeInTheDocument();
    await waitFor(() => {
      expect(mockApi.listJournalEntries).toHaveBeenCalledTimes(2);
    });
  });

  it("si guardar falla, avisa y deja el formulario abierto", async () => {
    const user = userEvent.setup();
    mockApi.listJournalEntries.mockResolvedValue([entry()]);
    mockApi.upsertJournalEntry.mockRejectedValue(new Error("content must not be empty"));

    render(<JournalView />);

    await user.click(await screen.findByRole("button", { name: /^Editar$/ }));
    await user.click(screen.getByRole("button", { name: /^Guardar$/ }));

    expect(
      await screen.findByText("content must not be empty"),
    ).toBeInTheDocument();
    // El formulario sigue abierto con lo escrito. No se comprueba el nombre del
    // botón «Guardar»: al pasar `saving` a `false`, el icono de carga de antd se
    // va con una animación que en jsdom nunca termina, y mientras sigue montado
    // suma su `aria-label` al nombre accesible del botón («loadingGuardar»).
    expect(screen.getByLabelText("Relato")).toHaveValue("un día largo");
  });

  it("borrar pide confirmación antes de llamar a la API", async () => {
    const user = userEvent.setup();
    mockApi.listJournalEntries.mockResolvedValueOnce([entry()]).mockResolvedValueOnce([]);
    mockApi.deleteJournalEntry.mockResolvedValue(undefined);

    render(<JournalView />);

    await user.click(await screen.findByRole("button", { name: /^Borrar$/ }));

    expect(
      await screen.findByText(/¿Borrar la entrada del 18 de octubre de 2026\?/),
    ).toBeInTheDocument();
    expect(mockApi.deleteJournalEntry).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /^Sí$/ }));

    await waitFor(() => {
      expect(mockApi.deleteJournalEntry).toHaveBeenCalledWith("2026-10-18");
    });
    // El texto del estado vacío empieza en mayúscula («Aparecerán…»): la
    // búsqueda es insensible a la caja para no atarla al inicio de la frase.
    expect(
      await screen.findByText(/aparecerán a medida que el agente/i),
    ).toBeInTheDocument();
  });

  it("cancelar la confirmación no borra la entrada", async () => {
    const user = userEvent.setup();
    mockApi.listJournalEntries.mockResolvedValue([entry()]);

    render(<JournalView />);

    await user.click(await screen.findByRole("button", { name: /^Borrar$/ }));
    await user.click(await screen.findByRole("button", { name: /^No$/ }));

    expect(mockApi.deleteJournalEntry).not.toHaveBeenCalled();
    expect(screen.getByText("un día largo")).toBeInTheDocument();
  });

  it("el borrado se puede confirmar solo con el teclado", async () => {
    const user = userEvent.setup();
    mockApi.listJournalEntries.mockResolvedValue([entry()]);
    mockApi.deleteJournalEntry.mockResolvedValue(undefined);

    render(<JournalView />);

    const borrar = await screen.findByRole("button", { name: /^Borrar$/ });
    borrar.focus();
    await user.keyboard("{Enter}");

    const si = await screen.findByRole("button", { name: /^Sí$/ });
    expect(mockApi.deleteJournalEntry).not.toHaveBeenCalled();

    si.focus();
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(mockApi.deleteJournalEntry).toHaveBeenCalledWith("2026-10-18");
    });
  });

  it("sin entradas muestra el estado vacío", async () => {
    mockApi.listJournalEntries.mockResolvedValue([]);

    render(<JournalView />);

    expect(
      await screen.findByText(/aparecerán a medida que el agente/i),
    ).toBeInTheDocument();
  });

  it("muestra un estado de carga mientras la petición está en curso", () => {
    mockApi.listJournalEntries.mockReturnValue(new Promise(() => {}));

    render(<JournalView />);

    expect(screen.getByRole("status")).toHaveTextContent(/cargando la bitácora/i);
    expect(screen.queryByText("un día largo")).not.toBeInTheDocument();
  });

  it("muestra un estado de error si la petición falla", async () => {
    mockApi.listJournalEntries.mockRejectedValue(new Error("no journal entry"));

    render(<JournalView />);

    expect(
      await screen.findByText("No se pudo cargar la bitácora"),
    ).toBeInTheDocument();
    expect(screen.getByText("no journal entry")).toBeInTheDocument();
  });
});
