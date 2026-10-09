import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import { useState, type ReactNode } from "react";

// ---------------------------------------------------------------------------
// Ant Design matchMedia mock
// ---------------------------------------------------------------------------
beforeEach(() => {
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

vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      getPersistentMemory: vi.fn(),
      updatePersistentMemory: vi.fn(),
      clearPersistentMemory: vi.fn(),
    },
  };
});

import { api, ApiError } from "../api/client";
import { PersistentMemoryPanel } from "../components/PersistentMemoryPanel";

const mockGet = vi.mocked(api.getPersistentMemory);
const mockUpdate = vi.mocked(api.updatePersistentMemory);
const mockClear = vi.mocked(api.clearPersistentMemory);

const stateFixture = {
  payload: { schema_version: 1, user_profile: { name: "Lorenzo" } },
  updated_at: "2026-10-01T10:00:00Z",
  token_count: 120,
  budget_tokens: 500,
  ceiling_tokens: 1000,
  is_empty: false,
};

const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

/**
 * Wrapper con estado que actualiza el prop `settings` al guardar, para poder
 * comprobar que la comparación de tokens refleja el presupuesto vigente.
 */
function SettingsHarness() {
  const [settings, setSettings] = useState<Record<string, string>>({
    PERSISTENT_MEMORY_BUDGET_TOKENS: "500",
  });
  return (
    <PersistentMemoryPanel
      settings={settings}
      savingSettings={false}
      updateSettings={async (data) => setSettings(data)}
    />
  );
}

function renderPanel(
  props: Partial<React.ComponentProps<typeof PersistentMemoryPanel>> = {},
) {
  const updateSettings = vi.fn().mockResolvedValue(undefined);
  const utils = render(
    <PersistentMemoryPanel
      settings={{ PERSISTENT_MEMORY_BUDGET_TOKENS: "500" }}
      updateSettings={updateSettings}
      savingSettings={false}
      {...props}
    />,
    { wrapper: AppWrapper },
  );
  return { updateSettings, ...utils };
}

const waitForTextarea = () =>
  screen.findByLabelText("Estado persistente");
const saveButton = () =>
  screen.getByRole("button", { name: /^Guardar$/ });
const saveBudgetButton = () =>
  screen.getByRole("button", { name: /Guardar presupuesto/ });

describe("PersistentMemoryPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGet.mockResolvedValue(stateFixture);
  });

  it("carga el estado y muestra marca, tokens y JSON formateado", async () => {
    renderPanel();

    expect(await screen.findByText(/120 \/ 500 tokens/)).toBeInTheDocument();
    expect(screen.getByText(/2026-10-01T10:00:00Z/)).toBeInTheDocument();

    const textarea = (await waitForTextarea()) as HTMLTextAreaElement;
    expect(textarea.value).toContain('"schema_version": 1');
    expect(textarea.value).toContain("\n");
  });

  it("usa el presupuesto de settings para la comparación de tokens", async () => {
    renderPanel({ settings: { PERSISTENT_MEMORY_BUDGET_TOKENS: "800" } });
    expect(await screen.findByText(/120 \/ 800 tokens/)).toBeInTheDocument();
  });

  it("señala cuando los tokens superan el presupuesto", async () => {
    mockGet.mockResolvedValue({ ...stateFixture, token_count: 900 });
    renderPanel();
    expect(
      await screen.findByText(/Supera el presupuesto/),
    ).toBeInTheDocument();
  });

  it("guarda enviando el payload y el expected_updated_at y confirma", async () => {
    const user = userEvent.setup();
    mockUpdate.mockResolvedValue({ ...stateFixture, warning: null });
    renderPanel();

    const textarea = await waitForTextarea();
    fireEvent.change(textarea, {
      target: {
        value: '{"schema_version": 1, "user_profile": { "name": "Ana" }}',
      },
    });

    await user.click(saveButton());

    await waitFor(() => {
      expect(mockUpdate).toHaveBeenCalledWith(
        { schema_version: 1, user_profile: { name: "Ana" } },
        "2026-10-01T10:00:00Z",
      );
    });
    expect(
      await screen.findByText("Memoria persistente guardada"),
    ).toBeInTheDocument();
  });

  it("muestra el error de validación sin llamar al servidor", async () => {
    const user = userEvent.setup();
    renderPanel();

    const textarea = await waitForTextarea();
    fireEvent.change(textarea, { target: { value: "esto no es json" } });

    await user.click(saveButton());

    expect(
      await screen.findByText(/no es JSON válido/i),
    ).toBeInTheDocument();
    expect(mockUpdate).not.toHaveBeenCalled();
  });

  it("muestra el warning de tamaño devuelto por el servidor", async () => {
    const user = userEvent.setup();
    mockUpdate.mockResolvedValue({
      ...stateFixture,
      warning: "Supera el presupuesto vigente",
    });
    renderPanel();

    await waitForTextarea();
    await user.click(saveButton());

    expect(
      await screen.findByText("Supera el presupuesto vigente"),
    ).toBeInTheDocument();
  });

  it("ante 409 recarga el estado y avisa del cambio", async () => {
    const user = userEvent.setup();
    mockGet
      .mockResolvedValueOnce(stateFixture)
      .mockResolvedValueOnce({ ...stateFixture, updated_at: "T2" });
    mockUpdate.mockRejectedValue(new ApiError("conflicto", 409));
    renderPanel();

    await waitForTextarea();
    await user.click(saveButton());

    expect(await screen.findByText(/cambió/i)).toBeInTheDocument();
    await waitFor(() => expect(mockGet).toHaveBeenCalledTimes(2));
    expect(screen.getByText(/T2/)).toBeInTheDocument();
  });

  it("vacia el estado tras confirmar", async () => {
    const user = userEvent.setup();
    mockClear.mockResolvedValue(undefined);
    renderPanel();

    await waitForTextarea();
    await user.click(screen.getByRole("button", { name: /Vaciar/ }));
    await user.click(await screen.findByRole("button", { name: /^Sí$/ }));

    await waitFor(() => expect(mockClear).toHaveBeenCalledTimes(1));
    expect(
      await screen.findByText("Memoria persistente vaciada"),
    ).toBeInTheDocument();
  });

  it("editar el presupuesto guarda solo esa clave sin tocar el estado", async () => {
    const user = userEvent.setup();
    const { updateSettings } = renderPanel();

    await waitForTextarea();
    const input = screen.getByLabelText("PERSISTENT_MEMORY_BUDGET_TOKENS");
    fireEvent.change(input, { target: { value: "800" } });

    await user.click(saveBudgetButton());

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({
        PERSISTENT_MEMORY_BUDGET_TOKENS: "800",
      });
    });
    expect(mockUpdate).not.toHaveBeenCalled();
    expect(mockClear).not.toHaveBeenCalled();
  });

  it("refleja de inmediato el presupuesto editado en la comparación de tokens", async () => {
    render(<SettingsHarness />, { wrapper: AppWrapper });
    await waitForTextarea();

    const input = screen.getByLabelText("PERSISTENT_MEMORY_BUDGET_TOKENS");
    fireEvent.change(input, { target: { value: "800" } });

    expect(await screen.findByText(/120 \/ 800 tokens/)).toBeInTheDocument();
  });

  it("mantiene el presupuesto editado tras guardarlo (settings actualizado)", async () => {
    const user = userEvent.setup();
    render(<SettingsHarness />, { wrapper: AppWrapper });
    await waitForTextarea();

    const input = screen.getByLabelText("PERSISTENT_MEMORY_BUDGET_TOKENS");
    fireEvent.change(input, { target: { value: "800" } });
    await user.click(saveBudgetButton());

    expect(await screen.findByText(/120 \/ 800 tokens/)).toBeInTheDocument();
  });

  it("permite vaciar el campo de presupuesto mientras se escribe", async () => {
    renderPanel();
    await waitForTextarea();

    const input = screen.getByLabelText(
      "PERSISTENT_MEMORY_BUDGET_TOKENS",
    ) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "" } });

    expect(input.value).toBe("");
  });

  it("avisa al intentar guardar un presupuesto vacío", async () => {
    const user = userEvent.setup();
    const { updateSettings } = renderPanel();
    await waitForTextarea();

    const input = screen.getByLabelText("PERSISTENT_MEMORY_BUDGET_TOKENS");
    fireEvent.change(input, { target: { value: "" } });
    await user.click(saveBudgetButton());

    expect(
      await screen.findByText("Introduce un presupuesto válido"),
    ).toBeInTheDocument();
    expect(updateSettings).not.toHaveBeenCalled();
  });
});
