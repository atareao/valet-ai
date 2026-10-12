import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import type { ReactNode } from "react";

// Mock del módulo de API de claves de ingesta: se sustituyen las tres funciones
// por dobles controlables desde cada prueba. No se usa `importActual` porque el
// módulo es el objeto de este change (patrón de `NotificationsTab.test.tsx`,
// mock parcial del cliente).
vi.mock("../api/ingestKeys", () => ({
  listIngestKeys: vi.fn(),
  createIngestKey: vi.fn(),
  revokeIngestKey: vi.fn(),
}));

import {
  createIngestKey,
  listIngestKeys,
  revokeIngestKey,
} from "../api/ingestKeys";
import { IngestKeysSection } from "./IngestKeysSection";
import type { IngestKey } from "../types";

const mockList = vi.mocked(listIngestKeys);
const mockCreate = vi.mocked(createIngestKey);
const mockRevoke = vi.mocked(revokeIngestKey);

const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

const activeKey: IngestKey = {
  id: "key-1",
  name: "monitor",
  prefix: "val_ABCDEFGH",
  created_at: "2026-10-12T09:00:00Z",
  last_used_at: null,
  revoked_at: null,
};

const revokedKey: IngestKey = {
  ...activeKey,
  revoked_at: "2026-10-12T10:00:00Z",
};

const TOKEN = "val_ABCDEFGH-SECRETO-QUE-SOLO-SE-VE-UNA-VEZ";

const renderSection = (): void => {
  render(<IngestKeysSection />, { wrapper: AppWrapper });
};

describe("IngestKeysSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockList.mockResolvedValue([]);
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

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("crear una clave muestra el token una sola vez con el aviso", async () => {
    const user = userEvent.setup();
    mockCreate.mockResolvedValue({ ...activeKey, token: TOKEN });
    mockList.mockResolvedValueOnce([]).mockResolvedValue([activeKey]);

    renderSection();

    await user.type(
      screen.getByLabelText("Nombre de la aplicación"),
      "monitor",
    );
    await user.click(screen.getByRole("button", { name: "Crear clave" }));

    expect(await screen.findByText(TOKEN)).toBeInTheDocument();
    expect(
      screen.getByText(/no se volverá a mostrar/i),
    ).toBeInTheDocument();
  });

  it("la lista muestra el prefijo y nunca el token", async () => {
    mockList.mockResolvedValue([activeKey]);

    renderSection();

    expect(await screen.findByText("val_ABCDEFGH")).toBeInTheDocument();
    expect((document.body.textContent ?? "").includes("SECRETO")).toBe(false);
  });

  it("revocar deja la clave marcada como revocada", async () => {
    const user = userEvent.setup();
    mockList.mockResolvedValueOnce([activeKey]).mockResolvedValue([revokedKey]);
    mockRevoke.mockResolvedValue(undefined);

    renderSection();

    await screen.findByText("val_ABCDEFGH");
    await user.click(screen.getByRole("button", { name: "Revocar" }));
    await user.click(await screen.findByRole("button", { name: "Sí" }));

    await waitFor(() => {
      expect(mockRevoke).toHaveBeenCalledWith("key-1");
    });
    expect(await screen.findByText("Revocada")).toBeInTheDocument();
  });

  it("un fallo del backend al crear se comunica sin dejar la clave a medias", async () => {
    const user = userEvent.setup();
    mockCreate.mockRejectedValue(new Error("boom"));
    mockList.mockResolvedValue([]);

    renderSection();

    await user.type(
      screen.getByLabelText("Nombre de la aplicación"),
      "monitor",
    );
    await user.click(screen.getByRole("button", { name: "Crear clave" }));

    expect(
      await screen.findByText(/No se pudo crear la clave de ingesta/i),
    ).toBeInTheDocument();
    // La clave NO queda a medias en la lista: la lista sigue sin la fila.
    expect(screen.queryByText("monitor")).not.toBeInTheDocument();
    expect(mockList).toHaveBeenCalledTimes(1);
  });
});
