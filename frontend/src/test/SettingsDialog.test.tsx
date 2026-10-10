import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import { useState } from "react";
import type { ReactElement, ReactNode } from "react";
import type { SkillsResponse } from "../types";

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

const mockUpdateProfile = vi.fn();
const mockUpdateSettings = vi.fn();
const mockResetToDefaults = vi.fn();
// `useSkills` (consumido por SkillsTab) es real: llama a `api.getSkills()`. Se
// mockea el cliente API parcialmente para controlar el catálogo por test. El
// prefijo `mock` permite referenciarlo desde el factory hoisted de `vi.mock`,
// igual que el resto de spies de este fichero.
const mockGetSkills = vi.fn();
// Spy sobre la lectura de settings del `RouterControl` autocontenido: permite
// contar las relecturas (p. ej. al remontarlo) además de servir el fixture.
const mockGetSettings = vi.fn();

// Fixture de settings devuelta por el hook mockeado (equivale a lo que
// responde `GET /settings`). Es mutable para que cada test pueda simular
// una respuesta distinta sin redefinir el módulo.
const defaultSettings: Record<string, string> = {
  max_window_tokens: "10000",
  system_prompt: "Eres Valet",
  archivist_prompt: "Eres un archivista",
  collapse_prompt: "Resume el texto",
  consolidator_prompt:
    "Consolida {{ ESTADO_ACTUAL }} con {{ BLOQUE_DE_MENSAJES }}",
  font_size: "16",
  message_page_size: "50",
  openweather_api_key: "",
  google_places_api_key: "",
  brave_search_api_key: "",
  MEMORY_HALF_LIFE_DAYS: "30",
  SIMILARITY_THRESHOLD: "0.4",
  RAG_BUDGET_TOKENS: "400",
  MEMORY_KNN_CANDIDATES: "10",
  GENERATION_CHAT_TEMPERATURE: "0.7",
  GENERATION_CHAT_REASONING: "",
  GENERATION_CHAT_MAX_TOKENS: "4096",
  GENERATION_COLLAPSE_TEMPERATURE: "0.2",
  GENERATION_COLLAPSE_REASONING: "off",
  GENERATION_COLLAPSE_MAX_TOKENS: "1024",
  GENERATION_MEMORY_TEMPERATURE: "0.3",
  GENERATION_MEMORY_REASONING: "off",
  GENERATION_MEMORY_MAX_TOKENS: "1024",
  GENERATION_SEMANTIC_TEMPERATURE: "0.1",
  GENERATION_SEMANTIC_REASONING: "low",
  GENERATION_SEMANTIC_MAX_TOKENS: "2048",
  ROUTER_ENABLED: "false",
  ROUTER_MODEL: "typesafe/jev-1.13",
  ROUTER_TIMEOUT_MS: "800",
  ROUTER_HISTORY_TURNS: "2",
  SKILL_AGENDA_PROMPT: "Fragmento de agenda",
  SKILL_PENDIENTES_PROMPT: "Fragmento de pendientes",
};

let mockSettings: Record<string, string> = { ...defaultSettings };

// Catálogo cerrado devuelto por `GET /api/skills` (forma real): las seis skills
// del catálogo, todas habilitadas, con sus valores efectivos y, en `agenda`,
// una pregunta sobrescrita. `core_tools` sigue presente porque el tipo lo exige.
const skillsFixture: SkillsResponse = {
  skills: [
    {
      id: "agenda",
      prompt_key: "SKILL_AGENDA_PROMPT",
      prompt_heading: "# SKILL ACTIVA: AGENDA",
      tools: ["calendar"],
      enabled: true,
      question: "¿La respuesta requiere mirar o cambiar la agenda?",
      criteria_true: "El mensaje se refiere a eventos o citas.",
      criteria_false: "El mensaje no se refiere a nada programado.",
      threshold: 0.1,
      overridden: ["question"],
    },
    {
      id: "pendientes",
      prompt_key: "SKILL_PENDIENTES_PROMPT",
      prompt_heading: "# SKILL ACTIVA: PENDIENTES",
      tools: ["tasks", "reminders"],
      enabled: true,
      question: "¿La respuesta requiere gestionar tareas o recordatorios?",
      criteria_true: "El mensaje se refiere a tareas o pendientes.",
      criteria_false: "El mensaje no se refiere a pendientes.",
      threshold: 0.1,
      overridden: [],
    },
    {
      id: "recuerdos",
      prompt_key: "SKILL_RECUERDOS_PROMPT",
      prompt_heading: "# SKILL ACTIVA: RECUERDOS",
      tools: ["notes", "unified_search"],
      enabled: true,
      question: "¿La respuesta requiere guardar o recuperar notas?",
      criteria_true: "El mensaje pide apuntar o recuperar un texto.",
      criteria_false: "El mensaje no pide guardar ni recuperar información.",
      threshold: 0.1,
      overridden: [],
    },
    {
      id: "entorno",
      prompt_key: "SKILL_ENTORNO_PROMPT",
      prompt_heading: "# SKILL ACTIVA: ENTORNO",
      tools: ["weather", "geocode", "reverse_geocode", "search_places"],
      enabled: true,
      question: "¿La respuesta requiere el tiempo o un lugar?",
      criteria_true: "El mensaje pregunta por el tiempo o por lugares.",
      criteria_false: "El mensaje no pregunta por el tiempo ni por lugares.",
      threshold: 0.1,
      overridden: [],
    },
    {
      id: "web",
      prompt_key: "SKILL_WEB_PROMPT",
      prompt_heading: "# SKILL ACTIVA: WEB",
      tools: ["web_search"],
      enabled: true,
      question: "¿La respuesta requiere información externa de internet?",
      criteria_true:
        "El mensaje pide información que no está en los datos del usuario.",
      criteria_false: "El mensaje se responde con datos del propio usuario.",
      threshold: 0.1,
      overridden: [],
    },
    {
      id: "widgets",
      prompt_key: "SKILL_WIDGETS_PROMPT",
      prompt_heading: "# SKILL ACTIVA: WIDGETS",
      tools: ["render_widget"],
      enabled: true,
      question: "¿La respuesta requiere mostrar algo interactivo?",
      criteria_true: "El turno implica pedir varios datos a la vez.",
      criteria_false: "El turno se resuelve con una explicación o un dato.",
      threshold: 0.2,
      overridden: [],
    },
  ],
  core_tools: ["render_widget", "get_current_time"],
};

// ---------------------------------------------------------------------------
// Mock hooks
// ---------------------------------------------------------------------------
vi.mock("../hooks/useProfile", () => ({
  useProfile: vi.fn(() => ({
    profile: { id: "test", name: "Test User", avatar_url: "", preferences: "{}" },
    loading: false,
    error: null,
    updateProfile: mockUpdateProfile,
  })),
}));

vi.mock("../hooks/useSettings", () => ({
  useSettings: vi.fn(() => ({
    settings: mockSettings,
    loading: false,
    saving: false,
    error: null,
    // Reproduce el comportamiento real del hook: el PUT refresca `settings`
    // con las claves enviadas. Así un `SkillsTab` que guarda vía este
    // `updateSettings` ve el valor persistido al re-sembrar.
    updateSettings: async (data: Record<string, string>) => {
      const result = await mockUpdateSettings(data);
      mockSettings = { ...mockSettings, ...data };
      return result;
    },
    resetToDefaults: mockResetToDefaults,
  })),
}));

// El panel de memoria persistente monta su propio hook; se mockea para que
// abrir su pestaña sea determinista y no dispare `api.getPersistentMemory()`.
vi.mock("../hooks/usePersistentMemory", () => ({
  usePersistentMemory: vi.fn(() => ({
    state: {
      payload: null,
      updated_at: null,
      token_count: 0,
      budget_tokens: 500,
      ceiling_tokens: 1000,
      is_empty: true,
    },
    loading: false,
    saving: false,
    error: null,
    warning: null,
    conflict: null,
    save: vi.fn(),
    clear: vi.fn(),
  })),
}));

// `useSkills` es real (no se mockea el hook): llama a `api.getSkills()`. Se
// sustituye solo esa función manteniendo el resto del cliente con
// `importActual` —los demás endpoints nunca se invocan porque sus hooks sí
// están mockeados—. `RouterControl` es autocontenido: llama directamente a
// `api.getSettings()` / `api.updateSettings()`, así que se enrutan al mismo
// fixture (`mockSettings`) y al mismo spy (`mockUpdateSettings`) que usa el
// resto del suite; las aserciones no cambian.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      // Diferido a tiempo de llamada: el factory de `vi.mock` se hoisted y se
      // evalúa antes de que `const mockGetSkills` exista. La función anidada
      // solo toca el spy cuando `useSkills` invoca `api.getSkills()`.
      getSkills: (...args: unknown[]) => mockGetSkills(...args),
      // Mismo diferido que `getSkills`: el factory se hoisted y solo toca el
      // spy cuando `RouterControl` invoca `api.getSettings()`.
      getSettings: () => mockGetSettings(),
      updateSettings: (data: Record<string, string>) => {
        // Propaga el resultado del spy: si un test lo hace rechazar, el
        // guardado falla de verdad (camino de error de «Restaurar»).
        const result = mockUpdateSettings(data);
        return Promise.resolve(result).then(() => mockSettings);
      },
    },
  };
});

import { SettingsDialog } from "../components/SettingsDialog";
import { RouterControl } from "../components/RouterControl";
import { ProfileProvider } from "../contexts/ProfileProvider";

// antd `App.useApp()` exige un `<App>` ancestro. Sin él el contexto por defecto
// son objetos vacíos: `messageApi.success` sería `undefined` y lanzaría un
// TypeError (fallo ruidoso, no un fallback silencioso a la API estática).
// Todo montaje va envuelto para ejercitar el camino contextual.
const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

const renderDialog = (ui: ReactElement) => render(ui, { wrapper: AppWrapper });

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // El mock del cliente propaga el rechazo del spy: se reinicia la
    // implementación de `mockUpdateSettings` para que un `mockRejectedValue`
    // de un test no se filtre al siguiente.
    mockUpdateSettings.mockReset();
    mockSettings = { ...defaultSettings };
    mockGetSkills.mockResolvedValue(skillsFixture);
    mockGetSettings.mockImplementation(() => Promise.resolve(mockSettings));
  });

  it("is not visible when visible=false", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={false} onClose={vi.fn()} /></ProfileProvider>);
    expect(screen.queryByText("⚙️ Settings")).not.toBeInTheDocument();
  });

  it("renders modal with correct title when visible", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);
    expect(screen.getByText("⚙️ Settings")).toBeInTheDocument();
  });

  it("renders Perfil tab by default with name and avatar fields", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);
    expect(screen.getByText("Perfil")).toBeInTheDocument();
    expect(screen.getByLabelText("Nombre")).toBeInTheDocument();
    expect(screen.getByLabelText("Avatar URL")).toBeInTheDocument();
  });

  it("saves profile when submitting Perfil tab", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    const nameInput = screen.getByLabelText("Nombre");
    await user.clear(nameInput);
    await user.type(nameInput, "Juan");

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateProfile).toHaveBeenCalledWith(
        expect.objectContaining({ name: "Juan" }),
      );
    });

    // REFACTOR (tarea 2.3): el aviso se asevera sobre el DOM — `<App>` lo
    // renderiza dentro del contenedor de RTL.
    expect(await screen.findByText("Perfil actualizado")).toBeInTheDocument();
  });

  it("shows error message when profile save fails", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockRejectedValue(new Error("fail"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(screen.getByText("Error al actualizar perfil")).toBeInTheDocument();
    });
  });

  it("renders Interfaz tab with font size, context window, and page size", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Click on Interfaz tab
    await user.click(screen.getByText("Interfaz"));

    expect(screen.getByText("Tamaño de fuente")).toBeInTheDocument();
    expect(screen.getByText("Ventana de contexto (tokens)")).toBeInTheDocument();
    expect(screen.getByText("Tamaño de página")).toBeInTheDocument();
  });

  it("renders Prompts tab with System, Archivist, Collapse and Consolidator sub-tabs", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));

    expect(screen.getByText("System")).toBeInTheDocument();
    expect(screen.getByText("Archivist")).toBeInTheDocument();
    expect(screen.getByText("Collapse")).toBeInTheDocument();
    expect(screen.getByText("Consolidator")).toBeInTheDocument();
  });

  it("shows system_prompt when opening the System sub-tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("System"));

    expect(screen.getByLabelText("System Prompt")).toHaveValue("Eres Valet");
  });

  it("shows archivist_prompt when opening the Archivist sub-tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Archivist"));

    expect(screen.getByLabelText("Archivist Prompt")).toHaveValue(
      "Eres un archivista",
    );
  });

  it("shows collapse_prompt when opening the Collapse sub-tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Collapse"));

    expect(screen.getByLabelText("Collapse Prompt")).toHaveValue(
      "Resume el texto",
    );
  });

  it("saves the three prompts together via updateSettings", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));

    const system = screen.getByLabelText("System Prompt");
    await user.clear(system);
    await user.type(system, "Nuevo system");

    const archivist = screen.getByLabelText("Archivist Prompt");
    await user.clear(archivist);
    await user.type(archivist, "Nuevo archivist");

    const collapse = screen.getByLabelText("Collapse Prompt");
    await user.clear(collapse);
    await user.type(collapse, "Nuevo collapse");

    const form = system.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          system_prompt: "Nuevo system",
          archivist_prompt: "Nuevo archivist",
          collapse_prompt: "Nuevo collapse",
        }),
      );
    });

    expect(await screen.findByText("Ajustes guardados")).toBeInTheDocument();
  });

  it("keeps the loaded prompts when saving from the Interfaz tab", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Interfaz"));

    const fontSize = screen.getByLabelText("Tamaño de fuente");
    await user.clear(fontSize);
    await user.type(fontSize, "20");

    const form = fontSize.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          font_size: "20",
          system_prompt: "Eres Valet",
          archivist_prompt: "Eres un archivista",
          collapse_prompt: "Resume el texto",
        }),
      );
    });
  });

  it("keeps interface settings when saving from the Prompts tab", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));

    const system = screen.getByLabelText("System Prompt");
    const form = system.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          font_size: "16",
          max_window_tokens: "10000",
          message_page_size: "50",
          system_prompt: "Eres Valet",
          archivist_prompt: "Eres un archivista",
          collapse_prompt: "Resume el texto",
        }),
      );
    });
  });

  it("editing only the Archivist prompt keeps the other prompts unchanged", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Archivist"));

    const archivist = screen.getByLabelText("Archivist Prompt");
    await user.clear(archivist);
    await user.type(archivist, "Nuevo archivist");

    const form = archivist.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          archivist_prompt: "Nuevo archivist",
          system_prompt: "Eres Valet",
          collapse_prompt: "Resume el texto",
        }),
      );
    });
  });

  it("renders API Keys tab with three password fields", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("API Keys"));

    expect(screen.getByText("OpenWeatherMap API Key")).toBeInTheDocument();
    expect(screen.getByText("Google Places API Key")).toBeInTheDocument();
    expect(screen.getByText("Brave Search API Key")).toBeInTheDocument();
  });

  it("calls resetToDefaults when clicking restore button", async () => {
    const user = userEvent.setup();
    mockResetToDefaults.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Interfaz"));

    await user.click(screen.getByRole("button", { name: /restaurar/i }));

    await waitFor(() => {
      expect(mockResetToDefaults).toHaveBeenCalled();
    });

    expect(
      await screen.findByText("Valores por defecto restaurados"),
    ).toBeInTheDocument();
  });

  it("shows error message when settings save fails", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockRejectedValue(new Error("boom"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Interfaz"));

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(
        screen.getByText("Error al guardar ajustes"),
      ).toBeInTheDocument();
    });
  });

  it("shows error message when resetToDefaults fails", async () => {
    const user = userEvent.setup();
    mockResetToDefaults.mockRejectedValue(new Error("boom"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Interfaz"));

    await user.click(screen.getByRole("button", { name: /restaurar/i }));

    await waitFor(() => {
      expect(
        screen.getByText("Error al restaurar valores"),
      ).toBeInTheDocument();
    });
  });

  it("calls onClose when modal is cancelled", async () => {
    const onClose = vi.fn();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={onClose} /></ProfileProvider>);

    // For antd Modal, the close button has aria-label "Close"
    const closeButton = screen.getByLabelText("Close");
    await userEvent.setup().click(closeButton);

    await waitFor(() => {
      expect(onClose).toHaveBeenCalled();
    });
  });

  // ════════════════════════════════════════════════════════════════
  // RED phase tests — validación de esquema de "Avatar URL"
  // ════════════════════════════════════════════════════════════════

  it("muestra error de validación y NO guarda con un esquema no permitido", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    await user.clear(avatarInput);
    await user.type(avatarInput, "javascript:alert(1)");

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(
        document.querySelector(".ant-form-item-explain-error"),
      ).not.toBeNull();
    });
    expect(mockUpdateProfile).not.toHaveBeenCalled();
  });

  it("guarda una URL https válida sin error de validación", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    await user.clear(avatarInput);
    await user.type(avatarInput, "https://example.com/me.png");

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateProfile).toHaveBeenCalledWith(
        expect.objectContaining({ avatar_url: "https://example.com/me.png" }),
      );
    });
    expect(document.querySelector(".ant-form-item-explain-error")).toBeNull();
  });

  it("acepta Avatar URL vacío y guarda sin error de validación", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    await user.clear(avatarInput);

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateProfile).toHaveBeenCalled();
    });
    expect(document.querySelector(".ant-form-item-explain-error")).toBeNull();
  });

  it("acepta una ruta relativa en Avatar URL y la guarda", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    await user.clear(avatarInput);
    await user.type(avatarInput, "/avatars/me.png");

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateProfile).toHaveBeenCalledWith(
        expect.objectContaining({ avatar_url: "/avatars/me.png" }),
      );
    });
    expect(document.querySelector(".ant-form-item-explain-error")).toBeNull();
  });

  it("persiste el Avatar URL recortado de espacios", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    await user.clear(avatarInput);
    await user.type(avatarInput, "  https://example.com/me.png  ");

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateProfile).toHaveBeenCalledWith(
        expect.objectContaining({ avatar_url: "https://example.com/me.png" }),
      );
    });
  });

  it("rechaza una URL relativa al protocolo", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    await user.clear(avatarInput);
    await user.type(avatarInput, "//evil.com/a.png");

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(
        document.querySelector(".ant-form-item-explain-error"),
      ).not.toBeNull();
    });
    expect(mockUpdateProfile).not.toHaveBeenCalled();
  });

  it("rechaza un valor con un tabulador embebido", async () => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    fireEvent.change(avatarInput, { target: { value: "java\tscript:alert(1)" } });

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(
        document.querySelector(".ant-form-item-explain-error"),
      ).not.toBeNull();
    });
    expect(mockUpdateProfile).not.toHaveBeenCalled();
  });

  it.each([
    "javascript:alert(1)",
    "data:image/png;base64,AAA",
    "file:///etc/passwd",
    "ftp://x/a.png",
    "//evil.com/a.png",
  ])("rechaza el esquema/valor no permitido %s", async (value) => {
    const user = userEvent.setup();
    mockUpdateProfile.mockResolvedValue(undefined);
    renderDialog(
      <ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>,
    );

    const avatarInput = screen.getByLabelText("Avatar URL");
    fireEvent.change(avatarInput, { target: { value } });

    await user.click(screen.getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(
        document.querySelector(".ant-form-item-explain-error"),
      ).not.toBeNull();
    });
    expect(mockUpdateProfile).not.toHaveBeenCalled();
  });

  // ════════════════════════════════════════════════════════════════
  // RED phase tests — pestaña "Memoria" (cuatro mandos numéricos)
  // change `settings-memory-tabs`: los mandos viven en la sub-pestaña
  // «Episódica» del Tabs anidado que agrupa «Memoria».
  // ════════════════════════════════════════════════════════════════

  // Abre la pestaña superior «Memoria» y activa la sub-pestaña «Episódica».
  const openEpisodicMemory = async (
    user: ReturnType<typeof userEvent.setup>,
  ) => {
    await user.click(screen.getByRole("tab", { name: "Memoria" }));
    await user.click(screen.getByRole("tab", { name: "Episódica" }));
  };

  it("renders the Memoria tab with the four memory knobs", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openEpisodicMemory(user);

    expect(screen.getByLabelText("MEMORY_HALF_LIFE_DAYS")).toBeInTheDocument();
    expect(screen.getByLabelText("SIMILARITY_THRESHOLD")).toBeInTheDocument();
    expect(screen.getByLabelText("RAG_BUDGET_TOKENS")).toBeInTheDocument();
    expect(screen.getByLabelText("MEMORY_KNN_CANDIDATES")).toBeInTheDocument();
  });

  it("loads the memory knob values from the settings response", async () => {
    const user = userEvent.setup();
    mockSettings = {
      ...mockSettings,
      MEMORY_HALF_LIFE_DAYS: "90",
      RAG_BUDGET_TOKENS: "800",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openEpisodicMemory(user);

    expect(screen.getByLabelText("MEMORY_HALF_LIFE_DAYS")).toHaveValue("90");
    expect(screen.getByLabelText("RAG_BUDGET_TOKENS")).toHaveValue("800");
  });

  it("saves the four memory knobs via updateSettings", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openEpisodicMemory(user);

    const halfLife = screen.getByLabelText("MEMORY_HALF_LIFE_DAYS");
    await user.clear(halfLife);
    await user.type(halfLife, "120");

    const threshold = screen.getByLabelText("SIMILARITY_THRESHOLD");
    await user.clear(threshold);
    await user.type(threshold, "0.7");

    const budget = screen.getByLabelText("RAG_BUDGET_TOKENS");
    await user.clear(budget);
    await user.type(budget, "1000");

    const knn = screen.getByLabelText("MEMORY_KNN_CANDIDATES");
    await user.clear(knn);
    await user.type(knn, "30");

    const form = halfLife.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          MEMORY_HALF_LIFE_DAYS: "120",
          SIMILARITY_THRESHOLD: "0.7",
          RAG_BUDGET_TOKENS: "1000",
          MEMORY_KNN_CANDIDATES: "30",
        }),
      );
    });

    expect(await screen.findByText("Ajustes guardados")).toBeInTheDocument();
  });

  it("renders the Episódica and Persistente sub-tabs inside Memoria", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Memoria" }));

    expect(screen.getByRole("tab", { name: "Episódica" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Persistente" })).toBeInTheDocument();
  });

  it("shows the nested memory Tabs inside a labelled region", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Memoria" }));

    const region = screen.getByRole("region", { name: "Tipo de memoria" });
    expect(region).toBeInTheDocument();
    expect(within(region).getByRole("tab", { name: "Episódica" })).toBeInTheDocument();
    expect(within(region).getByRole("tab", { name: "Persistente" })).toBeInTheDocument();
  });

  it("switching the memory sub-tab activates the persistent panel and deactivates Episódica", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // «Episódica» es la sub-pestaña activa por defecto. La actividad se
    // comprueba con `closest('[role="tabpanel"]')` + `aria-hidden` (jsdom no
    // emite `transitionend` y `toBeVisible()` daría false por la animación).
    await user.click(screen.getByRole("tab", { name: "Memoria" }));

    expect(screen.getByRole("tab", { name: "Episódica" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    await user.click(screen.getByRole("tab", { name: "Persistente" }));

    expect(screen.getByRole("tab", { name: "Persistente" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    const persistentBudget = await screen.findByLabelText(
      "PERSISTENT_MEMORY_BUDGET_TOKENS",
    );
    expect(persistentBudget.closest('[role="tabpanel"]')).toHaveAttribute(
      "aria-hidden",
      "false",
    );

    // El panel de «Episódica» queda desactivado aunque sus campos sigan en el
    // DOM (rc-tabs los conserva): el panel `role="tabpanel"` lleva
    // `aria-hidden="true"` cuando su sub-pestaña deja de estar seleccionada.
    const episodicPanel = screen
      .getByLabelText("MEMORY_HALF_LIFE_DAYS")
      .closest('[role="tabpanel"]');
    expect(episodicPanel).not.toBeNull();
    expect(episodicPanel).toHaveAttribute("aria-hidden", "true");
  });

  it("mounts the persistent memory panel only after its tab is reachable", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Memoria" }));

    // Escenario del delta: el panel solo se monta de forma perezosa al
    // seleccionar la sub-pestaña «Persistente».
    expect(
      screen.queryByLabelText("PERSISTENT_MEMORY_BUDGET_TOKENS"),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Persistente" }));

    expect(
      await screen.findByLabelText("PERSISTENT_MEMORY_BUDGET_TOKENS"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Estado persistente")).toBeInTheDocument();
  });

  // ════════════════════════════════════════════════════════════════
  // RED phase tests — change `settings-memory-tabs`: pestañas superiores
  // ════════════════════════════════════════════════════════════════

  it("shows exactly nine top-level tabs and no Herramientas tab", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    const topTabs = screen.getAllByRole("tab");

    expect(topTabs).toHaveLength(9);
    for (const name of [
      "Perfil",
      "Interfaz",
      "Prompts",
      "API Keys",
      "Memoria",
      "Generación",
      "Skills",
      "Enrutador de skills",
      "Integraciones",
    ]) {
      expect(screen.getByRole("tab", { name })).toBeInTheDocument();
    }
    // La antigua pestaña superior «Herramientas» ya no existe.
    expect(
      screen.queryByRole("tab", { name: "Herramientas" }),
    ).not.toBeInTheDocument();
    // La antigua pestaña superior «Memoria persistente» ya no existe.
    expect(
      screen.queryByRole("tab", { name: "Memoria persistente" }),
    ).not.toBeInTheDocument();
  });

  // ════════════════════════════════════════════════════════════════
  // RED phase tests — sub-pestaña "Consolidator" y aviso de placeholders
  // ════════════════════════════════════════════════════════════════

  it("renders the Consolidator sub-tab showing the current prompt", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    expect(screen.getByText("Consolidator")).toBeInTheDocument();

    await user.click(screen.getByText("Consolidator"));

    expect(screen.getByLabelText("Consolidator Prompt")).toHaveValue(
      "Consolida {{ ESTADO_ACTUAL }} con {{ BLOQUE_DE_MENSAJES }}",
    );
    expect(screen.queryByText(/Faltan placeholders/i)).not.toBeInTheDocument();
  });

  it("edits and saves consolidator_prompt keeping the rest of the settings", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Consolidator"));

    const area = screen.getByLabelText("Consolidator Prompt");
    await user.clear(area);
    await user.type(area, "Nuevo consolidator");

    const form = area.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          consolidator_prompt: "Nuevo consolidator",
          system_prompt: "Eres Valet",
          archivist_prompt: "Eres un archivista",
          collapse_prompt: "Resume el texto",
          font_size: "16",
          max_window_tokens: "10000",
        }),
      );
    });
  });

  it("warns about the missing placeholder naming it and still saves", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Consolidator"));

    const area = screen.getByLabelText("Consolidator Prompt");
    await user.clear(area);
    // user-event interpreta las llaves como descriptores de tecla; `fireEvent`
    // permite fijar el valor literal con placeholders.
    fireEvent.change(area, { target: { value: "Solo {{ ESTADO_ACTUAL }}" } });

    expect(
      await screen.findByText(/BLOQUE_DE_MENSAJES/),
    ).toBeInTheDocument();

    const form = area.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          consolidator_prompt: "Solo {{ ESTADO_ACTUAL }}",
        }),
      );
    });
  });

  it("does not warn when the prompt contains both placeholders", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Consolidator"));

    expect(screen.getByLabelText("Consolidator Prompt")).toHaveValue(
      "Consolida {{ ESTADO_ACTUAL }} con {{ BLOQUE_DE_MENSAJES }}",
    );
    expect(screen.queryByText(/Faltan placeholders/i)).not.toBeInTheDocument();
  });

  // ════════════════════════════════════════════════════════════════
  // RED phase tests — pestaña "Generación" (cuatro roles × tres mandos)
  // Escenario: SettingsDialog SHALL display a Generación tab with the
  // generation knobs (openspec/changes/generation-params/specs/frontend).
  // ════════════════════════════════════════════════════════════════

  // Los cuatro bloques y sus doce claves, en el orden del delta.
  const GENERATION_BLOCKS: Array<{
    heading: string;
    temperature: string;
    reasoning: string;
    maxTokens: string;
  }> = [
    {
      heading: "Chat",
      temperature: "GENERATION_CHAT_TEMPERATURE",
      reasoning: "GENERATION_CHAT_REASONING",
      maxTokens: "GENERATION_CHAT_MAX_TOKENS",
    },
    {
      heading: "Colapso",
      temperature: "GENERATION_COLLAPSE_TEMPERATURE",
      reasoning: "GENERATION_COLLAPSE_REASONING",
      maxTokens: "GENERATION_COLLAPSE_MAX_TOKENS",
    },
    {
      heading: "Fichas",
      temperature: "GENERATION_MEMORY_TEMPERATURE",
      reasoning: "GENERATION_MEMORY_REASONING",
      maxTokens: "GENERATION_MEMORY_MAX_TOKENS",
    },
    {
      heading: "Consolidación",
      temperature: "GENERATION_SEMANTIC_TEMPERATURE",
      reasoning: "GENERATION_SEMANTIC_REASONING",
      maxTokens: "GENERATION_SEMANTIC_MAX_TOKENS",
    },
  ];

  const REASONING_OPTIONS = [
    "default",
    "off",
    "minimal",
    "low",
    "medium",
    "high",
    "xhigh",
    "max",
  ];

  it("renders a Generación tab", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    expect(
      screen.getByRole("tab", { name: "Generación" }),
    ).toBeInTheDocument();
  });

  it("shows the four role blocks and their three generation fields", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    for (const block of GENERATION_BLOCKS) {
      expect(screen.getByText(block.heading)).toBeInTheDocument();
      expect(screen.getByLabelText(block.temperature)).toBeInTheDocument();
      expect(screen.getByLabelText(block.reasoning)).toBeInTheDocument();
      expect(screen.getByLabelText(block.maxTokens)).toBeInTheDocument();
    }
  });

  it("loads the generation values from the settings response", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    // El InputNumber formatea según el `step` (0,05 → dos decimales): "0.70".
    // Se compara numéricamente para no atar el test al formato de display.
    const chatTemperature = screen.getByLabelText(
      "GENERATION_CHAT_TEMPERATURE",
    ) as HTMLInputElement;
    expect(Number(chatTemperature.value)).toBeCloseTo(0.7);

    // El Select muestra el valor seleccionado en `.ant-select-selection-item`.
    const semanticReasoning = screen.getByLabelText(
      "GENERATION_SEMANTIC_REASONING",
    );
    const semanticSelect = semanticReasoning.closest(
      ".ant-select",
    ) as HTMLElement;
    expect(within(semanticSelect).getByText("low")).toBeInTheDocument();
  });

  it("offers the expected reasoning options in the generation selectors", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));
    await user.click(screen.getByLabelText("GENERATION_CHAT_REASONING"));

    for (const option of REASONING_OPTIONS) {
      expect(
        await screen.findByRole("option", { name: option }),
      ).toBeInTheDocument();
    }
  });

  it("saves the twelve generation knobs while keeping the other settings", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    const chatTemperature = screen.getByLabelText(
      "GENERATION_CHAT_TEMPERATURE",
    );
    await user.clear(chatTemperature);
    await user.type(chatTemperature, "0.9");

    const form = chatTemperature.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          GENERATION_CHAT_TEMPERATURE: "0.9",
          GENERATION_CHAT_REASONING: "",
          GENERATION_CHAT_MAX_TOKENS: "4096",
          GENERATION_COLLAPSE_TEMPERATURE: "0.2",
          GENERATION_COLLAPSE_REASONING: "off",
          GENERATION_COLLAPSE_MAX_TOKENS: "1024",
          GENERATION_MEMORY_TEMPERATURE: "0.3",
          GENERATION_MEMORY_REASONING: "off",
          GENERATION_MEMORY_MAX_TOKENS: "1024",
          GENERATION_SEMANTIC_TEMPERATURE: "0.1",
          GENERATION_SEMANTIC_REASONING: "low",
          GENERATION_SEMANTIC_MAX_TOKENS: "2048",
          // Las demás claves de settings no se pierden.
          system_prompt: "Eres Valet",
          font_size: "16",
          MEMORY_HALF_LIFE_DAYS: "30",
        }),
      );
    });

    expect(await screen.findByText("Ajustes guardados")).toBeInTheDocument();
  });

  // ════════════════════════════════════════════════════════════════
  // Contract tests — change `settings-dialog-layout`
  // Requisito: SettingsDialog SHALL organize the Generación roles in
  // sub-tabs + SHALL fit all its top-level tabs without overflow.
  // ════════════════════════════════════════════════════════════════

  it("shows the four role sub-tabs inside the Generación tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    // Los roles se exponen como etiquetas de sub-pestaña (role="tab").
    for (const role of ["Chat", "Colapso", "Fichas", "Consolidación"]) {
      expect(screen.getByRole("tab", { name: role })).toBeInTheDocument();
    }
  });

  it("switching the role sub-tab activates its fields and deactivates the previous ones", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // antd deja el Modal con `opacity: 0` por su animación de aparición y jsdom
    // no emite `transitionend`, así que `toBeVisible()` no sirve aquí (daría
    // false incluso para el panel activo). La actividad de una sub-pestaña se
    // comprueba con la señal estándar de rc-tabs: el panel `role="tabpanel"`
    // lleva `aria-hidden="false"` cuando está activo e `"true"` cuando no. Con
    // `forceRender` los campos de todos los paneles existen en el DOM.
    const panelOf = (labelText: string) =>
      screen.getByLabelText(labelText).closest('[role="tabpanel"]');
    const expectActive = (labelText: string) => {
      const panel = panelOf(labelText);
      expect(panel).not.toBeNull();
      expect(panel).toHaveAttribute("aria-hidden", "false");
    };
    const expectInactive = (labelText: string) => {
      const panel = panelOf(labelText);
      expect(panel).not.toBeNull();
      expect(panel).toHaveAttribute("aria-hidden", "true");
    };

    await user.click(screen.getByText("Generación"));

    // «Chat» es la sub-pestaña activa por defecto y sus campos cuelgan del
    // panel activo.
    expect(screen.getByRole("tab", { name: "Chat" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expectActive("GENERATION_CHAT_TEMPERATURE");

    await user.click(screen.getByRole("tab", { name: "Colapso" }));

    expect(screen.getByRole("tab", { name: "Colapso" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expectActive("GENERATION_COLLAPSE_TEMPERATURE");
    expectActive("GENERATION_COLLAPSE_REASONING");
    expectActive("GENERATION_COLLAPSE_MAX_TOKENS");

    // Los campos de «Chat» ya no están en el panel activo.
    expectInactive("GENERATION_CHAT_TEMPERATURE");
  });

  it("renders the settings modal with a width of at least 960px", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // antd v5 aplica el `width` como `style` inline del elemento `.ant-modal`,
    // que además es el que lleva role="dialog". Se lee el style inline para no
    // depender de getComputedStyle con pseudo-elementos (jsdom no lo implementa).
    const dialog = screen.getByRole("dialog") as HTMLElement;
    expect(parseFloat(dialog.style.width)).toBeGreaterThanOrEqual(960);
  });

  // ════════════════════════════════════════════════════════════════
  // change `skill-selection` — pestaña «Enrutador de skills» (control
  // global), pestaña superior «Skills» (configuración por skill) y avisos
  // no bloqueantes.
  // ════════════════════════════════════════════════════════════════

  const ROUTER_SWITCH = "Activar enrutado";

  // R1 — «Enrutador de skills»: el control global refleja el estado vigente.
  it("el control del enrutador refleja el estado vigente", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Enrutador de skills" }));

    expect(screen.getByRole("switch", { name: ROUTER_SWITCH })).not.toBeChecked();
    expect(screen.getByLabelText("Modelo de decisiones")).toHaveValue(
      "typesafe/jev-1.13",
    );
  });

  // R1 — encender y guardar persiste solo las claves del enrutador.
  it("encender el enrutador y guardar envía solo las claves del enrutador", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Enrutador de skills" }));
    await user.click(screen.getByRole("switch", { name: ROUTER_SWITCH }));

    const region = screen.getByRole("region", { name: "Enrutador de skills" });
    await user.click(within(region).getByRole("button", { name: "Guardar" }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledTimes(1);
    });
    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    // Exactamente las dos claves del enrutador: sin umbral global y sin las
    // claves por skill colándose en la petición.
    expect(Object.keys(payload).sort()).toEqual([
      "ROUTER_ENABLED",
      "ROUTER_MODEL",
    ]);
    expect(payload.ROUTER_ENABLED).toBe("true");
    expect(payload.ROUTER_MODEL).toBe("typesafe/jev-1.13");
    // No arrastra claves de otros grupos de settings.
    expect(payload).not.toHaveProperty("system_prompt");

    expect(
      await screen.findByText("Ajustes del enrutador guardados"),
    ).toBeInTheDocument();
  });

  // R1 — ni umbral global ni lista de herramientas en el control.
  it("no ofrece umbral global ni lista de herramientas en el control del enrutador", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Enrutador de skills" }));

    expect(
      screen.queryByLabelText("Umbral del enrutador"),
    ).not.toBeInTheDocument();
    // El único interruptor de la sección es el de activar el enrutado.
    const region = screen.getByRole("region", { name: "Enrutador de skills" });
    expect(within(region).getAllByRole("switch")).toHaveLength(1);
  });

  // R1 — el campo del modelo se identifica como modelo de decisiones.
  it("el campo del modelo se identifica como modelo de decisiones", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Enrutador de skills" }));

    expect(screen.getByLabelText("Modelo de decisiones")).toHaveValue(
      "typesafe/jev-1.13",
    );
    expect(screen.getByText(/no un modelo generativo/i)).toBeInTheDocument();
  });

  // R3 — aviso de enrutado inactivo sin bloquear el guardado.
  it("avisa de que el enrutado está inactivo cuando está apagado", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Enrutador de skills" }));

    expect(
      await screen.findByText(/El enrutador está apagado/),
    ).toBeInTheDocument();

    // El guardado sigue disponible pese al aviso.
    const region = screen.getByRole("region", { name: "Enrutador de skills" });
    await user.click(within(region).getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalled();
    });
  });

  // ════════════════════════════════════════════════════════════════
  // R2 — pestaña superior «Skills» (change `skill-selection`): una pestaña
  // por skill listada desde el catálogo, con habilitación, umbral y los cuatro
  // textos, marca y «restaurar» en los campos sobrescritos, guardado por diff
  // y aviso no bloqueante si un campo queda vacío.
  // ════════════════════════════════════════════════════════════════

  const SKILL_IDS = [
    "agenda",
    "pendientes",
    "recuerdos",
    "entorno",
    "web",
    "widgets",
  ] as const;

  const SKILL_FIELD_LABELS = [
    "Pregunta",
    "Criterio SÍ",
    "Criterio NO",
    "Fragmento de prompt",
  ] as const;

  // Nombres accesibles únicos: cada campo y cada botón incorpora el id de la
  // skill para no colisionar entre las pestañas.
  const fieldLabel = (id: string, label: string) => `${label} (${id})`;
  const restoreLabel = (id: string, label: string) =>
    `Restaurar ${id} · ${label}`;

  // Abre la pestaña superior «Skills» y espera a que el catálogo se resuelva
  // (una sub-pestaña por skill). La degradación no usa este helper.
  const openSkillsTab = async (
    user: ReturnType<typeof userEvent.setup>,
  ) => {
    await user.click(screen.getByRole("tab", { name: "Skills" }));
    await screen.findAllByRole("tab", { name: "agenda" });
  };

  // Panel de la sub-pestaña de una skill, localizado a partir de uno de sus
  // campos. Permite acotar las aserciones aunque los paneles inactivos sigan
  // en el DOM (rc-tabs los conserva con `aria-hidden`).
  const skillPane = (id: string): HTMLElement => {
    const field = screen.getByLabelText(fieldLabel(id, "Pregunta"));
    const pane = field.closest('[role="tabpanel"]');
    expect(pane).not.toBeNull();
    return pane as HTMLElement;
  };

  // Escenario: las seis skills figuran como pestañas, cada una con su
  // interruptor, su umbral y sus cuatro campos.
  it("lista las seis skills del catálogo, una pestaña por skill, con su interruptor y su umbral", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    for (const id of SKILL_IDS) {
      expect(screen.getByRole("tab", { name: id })).toBeInTheDocument();

      // La `Tabs` interna de `SkillsTab` usa `forceRender`: los paneles de
      // todas las skills ya están montados aunque los inactivos estén
      // ocultos. Consultamos sin clicar sub-pestañas (evita las animaciones
      // de rc-tabs, lentas en CI) pasando `hidden: true` donde la librería
      // filtra por accesibilidad.
      expect(
        screen.getByRole("switch", {
          name: `Habilitada ${id}`,
          hidden: true,
        }),
      ).toBeInTheDocument();
      expect(screen.getByLabelText(`Umbral de ${id}`)).toBeInTheDocument();
      for (const label of SKILL_FIELD_LABELS) {
        expect(screen.getByLabelText(fieldLabel(id, label))).toBeInTheDocument();
      }
    }

    // Nombres accesibles únicos entre pestañas: la etiqueta desnuda ya no
    // existe y la etiquetada con el id aparece exactamente una vez.
    expect(screen.queryByLabelText("Pregunta")).not.toBeInTheDocument();
    expect(
      screen.getAllByLabelText(fieldLabel("agenda", "Pregunta")),
    ).toHaveLength(1);
  });

  // Escenario: los valores vigentes efectivos se muestran.
  it("muestra el valor efectivo de la pregunta (catálogo) y del fragmento (settings)", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    expect(screen.getByLabelText(fieldLabel("agenda", "Pregunta"))).toHaveValue(
      "¿La respuesta requiere mirar o cambiar la agenda?",
    );
    expect(
      screen.getByLabelText(fieldLabel("agenda", "Fragmento de prompt")),
    ).toHaveValue("Fragmento de agenda");
    expect(
      screen.getByLabelText(fieldLabel("pendientes", "Fragmento de prompt")),
    ).toHaveValue("Fragmento de pendientes");
  });

  // Escenario: un campo sobrescrito se marca.
  it("marca el campo sobrescrito y ofrece restaurarlo", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    // `agenda` trae su pregunta sobrescrita (y su fragmento, por settings):
    // se marca y se puede restaurar.
    const agenda = skillPane("agenda");
    expect(within(agenda).getAllByText("Modificado").length).toBeGreaterThan(0);
    expect(
      within(agenda).getByRole("button", {
        name: restoreLabel("agenda", "Pregunta"),
        hidden: true,
      }),
    ).toBeInTheDocument();

    // Una skill sin sobrescrituras no muestra marca ni acción.
    const web = skillPane("web");
    expect(within(web).queryByText("Modificado")).not.toBeInTheDocument();
    expect(
      within(web).queryByRole("button", { name: /Restaurar/, hidden: true }),
    ).not.toBeInTheDocument();
  });

  // Escenario: restaurar el valor por defecto vacía su clave en settings.
  it("restaurar envía la clave con cadena vacía y relee el catálogo", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);
    const readsBefore = mockGetSkills.mock.calls.length;

    await user.click(
      screen.getByRole("button", {
        name: restoreLabel("agenda", "Pregunta"),
      }),
    );

    await waitFor(() => {
      // Vaciar la clave equivale a restaurar el valor por defecto.
      expect(mockUpdateSettings).toHaveBeenCalledWith({
        SKILL_AGENDA_QUESTION: "",
      });
    });
    await waitFor(() => {
      expect(mockGetSkills.mock.calls.length).toBeGreaterThan(readsBefore);
    });
    expect(
      await screen.findByText("Valor por defecto restaurado"),
    ).toBeInTheDocument();
  });

  // Escenario: si el guardado falla, la UI no miente sobre el valor del campo.
  it("mantiene el valor del campo y avisa si la restauración falla", async () => {
    const user = userEvent.setup();
    // El guardado de la restauración falla: `api.updateSettings` rechaza.
    mockUpdateSettings.mockRejectedValueOnce(new Error("boom"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    const question = screen.getByLabelText(fieldLabel("agenda", "Pregunta"));
    const valueBefore = (question as HTMLTextAreaElement).value;

    await user.click(
      screen.getByRole("button", {
        name: restoreLabel("agenda", "Pregunta"),
      }),
    );

    expect(
      await screen.findByText("Error al restaurar el valor"),
    ).toBeInTheDocument();
    // El valor mostrado no se toca: sigue siendo el efectivo sobrescrito y la
    // marca de «Modificado» permanece (la UI no refleja un guardado fallido).
    expect(screen.getByLabelText(fieldLabel("agenda", "Pregunta"))).toHaveValue(
      valueBefore,
    );
    expect(
      within(skillPane("agenda")).getAllByText("Modificado").length,
    ).toBeGreaterThan(0);
    expect(
      screen.queryByText("Valor por defecto restaurado"),
    ).not.toBeInTheDocument();
  });

  // Escenario: guardar un fragmento no toca el resto de settings.
  it("guardar envía solo el campo modificado", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    const fragment = screen.getByLabelText(
      fieldLabel("agenda", "Fragmento de prompt"),
    );
    await user.clear(fragment);
    await user.type(fragment, "Nuevo fragmento de agenda");

    const form = fragment.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          SKILL_AGENDA_PROMPT: "Nuevo fragmento de agenda",
        }),
      );
    });

    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    // Solo la clave del campo modificado: ni el resto de las claves de skill ni
    // las demás claves de settings.
    expect(Object.keys(payload)).toEqual(["SKILL_AGENDA_PROMPT"]);
  });

  // Regresión: tras guardar, el textarea del fragmento no revierte al valor
  // viejo. El guardado refresca `settings` (vía el hook) y el re-sembrado toma
  // el valor persistido.
  it("tras guardar un fragmento, el valor mostrado sigue siendo el guardado (no revierte)", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue({ ...mockSettings });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    const fragmentLabel = fieldLabel("agenda", "Fragmento de prompt");
    const fragment = screen.getByLabelText(fragmentLabel);
    await user.clear(fragment);
    await user.type(fragment, "Nuevo fragmento de agenda");

    const form = fragment.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ SKILL_AGENDA_PROMPT: "Nuevo fragmento de agenda" }),
      );
    });

    // El re-sembrado (disparado por la relectura del catálogo) toma `settings`
    // ya refrescado: el borrador persistido, no el valor obsoleto de partida.
    await waitFor(() => {
      expect(screen.getByLabelText(fragmentLabel)).toHaveValue(
        "Nuevo fragmento de agenda",
      );
    });
  });

  // Bug 2: el fragmento sobrescrito (clave de settings no vacía) se marca y
  // «Restaurar» la vacía para volver al valor del catálogo.
  it("marca el fragmento sobrescrito y «Restaurar» lo vacía en settings", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    // `SKILL_AGENDA_PROMPT` no vacío en settings ⇒ el fragmento se marca.
    const fragmentLabel = fieldLabel("agenda", "Fragmento de prompt");
    const fragment = screen.getByLabelText(fragmentLabel);
    const fragmentPane = fragment.closest('[role="tabpanel"]') as HTMLElement;
    expect(within(fragmentPane).getAllByText("Modificado").length).toBeGreaterThan(0);

    await user.click(
      screen.getByRole("button", {
        name: restoreLabel("agenda", "Fragmento de prompt"),
      }),
    );

    await waitFor(() => {
      // Vaciar la clave equivale a restaurar el fragmento.
      expect(mockUpdateSettings).toHaveBeenCalledWith({
        SKILL_AGENDA_PROMPT: "",
      });
    });
    expect(
      await screen.findByText("Valor por defecto restaurado"),
    ).toBeInTheDocument();
  });

  // Escenario: deshabilitar una skill se persiste.
  it("deshabilitar una skill y guardar envía ROUTER_SKILL_<ID>_ENABLED = false", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    const enabled = screen.getByRole("switch", { name: "Habilitada agenda" });
    expect(enabled).toBeChecked();

    await user.click(enabled);

    const form = enabled.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ ROUTER_SKILL_AGENDA_ENABLED: "false" }),
      );
    });
  });

  // Escenario: editar el umbral propio de una skill persiste su clave.
  it("editar el umbral de una skill persiste ROUTER_THRESHOLD_<ID>", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);
    await user.click(screen.getByRole("tab", { name: "widgets" }));

    const threshold = screen.getByLabelText("Umbral de widgets");
    await user.clear(threshold);
    await user.type(threshold, "0.35");

    const form = threshold.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ ROUTER_THRESHOLD_WIDGETS: "0.35" }),
      );
    });

    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    expect(payload.ROUTER_THRESHOLD_WIDGETS).toBe("0.35");
    // Las skills cuyo umbral no cambia no aportan clave propia.
    expect(payload).not.toHaveProperty("ROUTER_THRESHOLD_AGENDA");
  });

  // Escenario: aviso al guardar un campo vacío.
  it("avisa sin bloquear al guardar un campo de skill vacío", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await openSkillsTab(user);

    const question = screen.getByLabelText(fieldLabel("agenda", "Pregunta"));
    await user.clear(question);

    const form = question.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    // El guardado no se bloquea: se envía la clave vacía y avisa.
    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ SKILL_AGENDA_QUESTION: "" }),
      );
    });
    expect(await screen.findByText(/campos vacíos/i)).toBeInTheDocument();
    expect(
      await screen.findByText("Ajustes de skills guardados"),
    ).toBeInTheDocument();
  });

  // R3 — aviso con umbral extremo por skill sin bloquear el guardado.
  it.each([
    { value: 0, message: /desactiva el filtrado/i },
    { value: 1, message: /inalcanzable/i },
  ])(
    "avisa con umbral extremo ($value) por skill sin bloquear el guardado",
    async ({ value, message }) => {
      const user = userEvent.setup();
      mockUpdateSettings.mockResolvedValue(undefined);
      // El umbral mostrado sale del catálogo: se fuerza el valor extremo en la
      // skill `agenda` antes de montar.
      mockGetSkills.mockResolvedValue({
        ...skillsFixture,
        skills: skillsFixture.skills.map((skill) =>
          skill.id === "agenda" ? { ...skill, threshold: value } : skill,
        ),
      });
      renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

      await openSkillsTab(user);

      expect(await screen.findByText(message)).toBeInTheDocument();

      // El guardado sigue disponible pese al aviso.
      expect(
        screen.getByRole("button", { name: /guardar/i }),
      ).toBeInTheDocument();
    },
  );

  // Escenario: la vista degrada si el catálogo falla.
  it("degrada con un aviso si el catálogo de skills falla, sin romper el formulario", async () => {
    const user = userEvent.setup();
    mockGetSkills.mockRejectedValue(new Error("boom"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Sin catálogo no hay sub-pestañas de skill: se abre la pestaña superior.
    await user.click(screen.getByRole("tab", { name: "Skills" }));

    expect(
      await screen.findByText("No se pudieron cargar las skills"),
    ).toBeInTheDocument();
    // El resto del formulario sigue renderizando: sus prompts y su guardado.
    await user.click(screen.getByRole("tab", { name: "Prompts" }));
    expect(screen.getByLabelText("System Prompt")).toBeInTheDocument();
    const form = screen.getByLabelText("System Prompt").closest("form") as HTMLFormElement;
    expect(within(form).getByRole("button", { name: /guardar/i })).toBeInTheDocument();
  });

  // R1 — regresión: el borrador del enrutador se descarta al remontar el
  // control, que vuelve a leer lo persistido.
  //
  // El mecanismo real incrementa `routerOpenKey` desde el `afterOpenChange` del
  // `Modal` de antd. En jsdom ese callback NO dispara: `rc-motion` espera un
  // `transitionend` que jsdom nunca emite y `Dialog` no pasa `motionDeadline`,
  // así que la animación nunca «termina» y `onVisibleChanged` no se invoca. No
  // se fabrica un test que dependa de ese evento. Se cubre el comportamiento
  // observable equivalente: `RouterControl` remontado (lo que produce el cambio
  // de `key`) descarta la edición sin guardar y re-lee `getSettings`.
  it("remontar el control descarta la edición sin guardar y relee lo persistido", async () => {
    const user = userEvent.setup();
    mockSettings = { ...mockSettings, ROUTER_ENABLED: "false" };

    function RouterHarness() {
      const [mountKey, setMountKey] = useState(0);
      return (
        <>
          <button type="button" onClick={() => setMountKey((k) => k + 1)}>
            remount
          </button>
          <RouterControl key={mountKey} />
        </>
      );
    }

    render(<RouterHarness />, { wrapper: AppWrapper });

    const switchBefore = await screen.findByRole("switch", { name: ROUTER_SWITCH });
    expect(switchBefore).not.toBeChecked();

    // Encender sin guardar: el borrador vive solo en el estado local.
    await user.click(switchBefore);
    expect(screen.getByRole("switch", { name: ROUTER_SWITCH })).toBeChecked();
    expect(mockUpdateSettings).not.toHaveBeenCalled();

    const readsBefore = mockGetSettings.mock.calls.length;

    await user.click(screen.getByRole("button", { name: "remount" }));

    // El control remontado relee y pinta lo persistido (apagado), no el
    // borrador encendido.
    expect(
      await screen.findByRole("switch", { name: ROUTER_SWITCH }),
    ).not.toBeChecked();
    expect(mockGetSettings.mock.calls.length).toBeGreaterThan(readsBefore);
  });
});
