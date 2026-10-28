import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within, fireEvent, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import { useState } from "react";
import type { ReactElement, ReactNode } from "react";
import type { OpentodoCheckResult, SkillsResponse } from "../types";

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
// Spies sobre los endpoints dedicados de ZenNotes: la sección de token es
// autocontenida y no usa `updateSettings`, así que se controla el cliente para
// comprobar que guardar el token va a `PUT /api/zennotes/token` y nunca a
// `PUT /api/settings`.
const mockGetZennotesStatus = vi.fn();
const mockSetZennotesToken = vi.fn();
// Spy sobre `GET /api/strava/status`: `useStrava` es real (lo consume
// `StravaIntegration`), así que se enruta al spy para que el indicador de la
// sub-pestaña «Strava» sea determinista y ningún test abra una petición real.
const mockGetStravaStatus = vi.fn();
// Spies sobre los endpoints dedicados de OpenTodo: misma pauta que ZenNotes
// (change `opentodo-integration`). `useOpentodo` es real, así que se enruta al
// spy para que el indicador de la octava sub-pestaña sea determinista y ningún
// test abra una petición real.
const mockGetOpentodoStatus = vi.fn();
const mockSetOpentodoToken = vi.fn();
// Spy sobre el sondeo activo de OpenTodo (`GET /api/opentodo/check`): el botón
// «Probar conexión» debe usarlo —no una relectura del estado— y mostrar su
// `detail` en la línea de estado.
const mockCheckOpentodo = vi.fn();

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
  timeline_prompt: "Extrae hechos fechados",
  journal_prompt: "Cuenta el día como lo viviste",
  font_size: "16",
  message_page_size: "50",
  openweather_api_key: "",
  google_places_api_key: "",
  brave_search_api_key: "",
  apimail_base_url: "",
  apimail_api_key: "",
  miniflux_base_url: "",
  miniflux_api_key: "",
  opentodo_base_url: "",
  MEMORY_HALF_LIFE_DAYS: "30",
  SIMILARITY_THRESHOLD: "0.4",
  RAG_BUDGET_TOKENS: "400",
  MEMORY_KNN_CANDIDATES: "10",
  GENERATION_CHAT_TEMPERATURE: "0.7",
  GENERATION_CHAT_REASONING: "off",
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
  GENERATION_TIMELINE_TEMPERATURE: "0.2",
  GENERATION_TIMELINE_REASONING: "off",
  GENERATION_TIMELINE_MAX_TOKENS: "2048",
  REACT_TOOL_FINGERPRINT_LIMIT: "2",
  REACT_TOOL_FAILURE_LIMIT: "3",
  REACT_TOOL_SUCCESS_LIMIT: "50",
  REACT_TOOL_TOTAL_LIMIT: "100",
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

// `saving` del hook `useSettings`, mutable para que un test pueda simular el
// `PUT /api/settings` en vuelo: es lo que declara ocupados (`loading` +
// `aria-busy`) a los botones «Guardar» que dependen de él, incluido el del
// panel de OpenTodo, que lo recibe por el camino
// `SettingsDialog` → `IntegrationsTab` → `OpentodoSection`.
let mockSaving = false;

// El hook se mockea con estado real de React: `settings` vive en un
// `useState` (como en el hook verdadero), de modo que un guardado refresca el
// valor y repinta a los consumidores —por ejemplo el indicador de configuración
// de las sub-pestañas de «Integraciones»— sin recargar la página. El estado se
// siembra con `mockSettings`, que cada test ajusta antes de renderizar.
vi.mock("../hooks/useSettings", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  const useSettings = () => {
    const [settings, setSettings] = React.useState<Record<string, string>>(
      mockSettings,
    );
    // Reproduce el comportamiento real del hook: el PUT refresca `settings`
    // con las claves enviadas.
    const updateSettings = React.useCallback(
      async (data: Record<string, string>) => {
        const result = await mockUpdateSettings(data);
        mockSettings = { ...mockSettings, ...data };
        setSettings(mockSettings);
        return result;
      },
      [],
    );
    return {
      settings,
      loading: false,
      saving: mockSaving,
      error: null,
      updateSettings,
      resetToDefaults: mockResetToDefaults,
    };
  };
  return { useSettings: vi.fn(useSettings) };
});

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
      // Endpoints dedicados de ZenNotes: mismo diferido que `getSkills` (el
      // factory se hoisted); solo se toca el spy cuando el hook los invoca.
      getZennotesStatus: (...args: unknown[]) =>
        mockGetZennotesStatus(...args),
      setZennotesToken: (...args: unknown[]) => mockSetZennotesToken(...args),
      // Endpoints dedicados de OpenTodo: mismo diferido que `getSkills` (el
      // factory se hoisted); solo se toca el spy cuando el hook los invoca.
      getOpentodoStatus: (...args: unknown[]) => mockGetOpentodoStatus(...args),
      setOpentodoToken: (...args: unknown[]) => mockSetOpentodoToken(...args),
      checkOpentodo: (...args: unknown[]) => mockCheckOpentodo(...args),
      // Estado de Strava: mismo diferido. Lo consume `useStrava` al montar
      // `StravaIntegration` dentro de «Integraciones».
      getStravaStatus: (...args: unknown[]) => mockGetStravaStatus(...args),
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

/**
 * Termina la animación de cierre del `Modal` de ajustes.
 *
 * `destroyOnHidden` desmonta el contenido **al final** de la animación de
 * salida. En un navegador ese final llega con el `transitionend` del propio
 * `Modal`; jsdom no emite ese evento nunca, así que `rc-motion` se queda en
 * `leave-start` y el desmontaje no se observaría. Aquí se espera a que la
 * animación esté en su tramo activo y se notifica su final —un único evento,
 * sin temporizadores ni `afterOpenChange`—, que es exactamente lo que hace el
 * navegador. El mecanismo probado no depende de la animación: solo se le da
 * por terminada.
 */
async function finishModalClose(): Promise<void> {
  const panel = document.querySelector<HTMLElement>(".ant-modal");
  expect(panel).not.toBeNull();
  await waitFor(() => {
    expect(panel).toHaveClass("ant-zoom-leave-active");
  });
  fireEvent.transitionEnd(panel as HTMLElement);
  await waitFor(() => {
    expect(panel).not.toBeInTheDocument();
  });
}

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // El mock del cliente propaga el rechazo del spy: se reinicia la
    // implementación de `mockUpdateSettings` para que un `mockRejectedValue`
    // de un test no se filtre al siguiente.
    mockUpdateSettings.mockReset();
    mockSaving = false;
    mockSettings = { ...defaultSettings };
    mockGetSkills.mockResolvedValue(skillsFixture);
    mockGetSettings.mockImplementation(() => Promise.resolve(mockSettings));
    // Por defecto ZenNotes no está configurado; los tests que lo necesiten
    // fijan `configured: true` con `mockResolvedValueOnce`/`mockResolvedValue`.
    mockGetZennotesStatus.mockResolvedValue({
      configured: false,
      base_url: "https://zennotes.territoriolinux.es",
      folder: "Valet",
    });
    mockSetZennotesToken.mockResolvedValue(undefined);
    // Por defecto OpenTodo no está configurado; los tests que lo necesiten
    // fijan `configured: true` con `mockResolvedValueOnce`/`mockResolvedValue`.
    mockGetOpentodoStatus.mockResolvedValue({
      configured: false,
      base_url: "https://todo.territoriolinux.es",
    });
    mockSetOpentodoToken.mockResolvedValue(undefined);
    // Sondeo activo por defecto: «conexión correcta». Los tests de fallo lo
    // sobrescriben con `mockResolvedValue`/`mockRejectedValue`.
    mockCheckOpentodo.mockResolvedValue({
      ok: true,
      detail: "conexión correcta",
    });
    // Por defecto Strava no está conectada; los tests que lo necesiten fijan el
    // estado conectado con `mockResolvedValue`.
    mockGetStravaStatus.mockResolvedValue({
      connected: false,
      athlete_id: null,
      athlete_name: null,
      scope: null,
    });
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

  it("renders Prompts tab with System, Archivist, Collapse, Consolidator and Timeline sub-tabs", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));

    expect(screen.getByText("System")).toBeInTheDocument();
    expect(screen.getByText("Archivist")).toBeInTheDocument();
    expect(screen.getByText("Collapse")).toBeInTheDocument();
    expect(screen.getByText("Consolidator")).toBeInTheDocument();
    expect(screen.getByText("Timeline")).toBeInTheDocument();
  });

  it("shows timeline_prompt when opening the Timeline sub-tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Timeline"));

    expect(screen.getByLabelText("Timeline Prompt")).toHaveValue(
      "Extrae hechos fechados",
    );
  });

  it("saves timeline_prompt and the GENERATION_TIMELINE_* keys via updateSettings", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Timeline"));

    const area = screen.getByLabelText("Timeline Prompt");
    await user.clear(area);
    await user.type(area, "Nuevo timeline");

    const form = area.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          timeline_prompt: "Nuevo timeline",
          GENERATION_TIMELINE_TEMPERATURE: "0.2",
          GENERATION_TIMELINE_REASONING: "off",
          GENERATION_TIMELINE_MAX_TOKENS: "2048",
        }),
      );
    });

    // El guardado envía las quince claves `GENERATION_*` (cinco roles × tres).
    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    const generationKeys = Object.keys(payload).filter((key) =>
      key.startsWith("GENERATION_"),
    );
    expect(generationKeys).toHaveLength(15);
  });

  it("shows the Journal sub-tab in the Prompts tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));

    expect(screen.getByText("Journal")).toBeInTheDocument();
  });

  it("shows journal_prompt when opening the Journal sub-tab", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Journal"));

    expect(screen.getByLabelText("Journal Prompt")).toHaveValue(
      "Cuenta el día como lo viviste",
    );
  });

  it("saves journal_prompt via updateSettings without touching the other prompts", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Prompts"));
    await user.click(screen.getByText("Journal"));

    const area = screen.getByLabelText("Journal Prompt");
    await user.clear(area);
    await user.type(area, "Mi relato personalizado");

    const form = area.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          journal_prompt: "Mi relato personalizado",
          system_prompt: "Eres Valet",
          archivist_prompt: "Eres un archivista",
          collapse_prompt: "Resume el texto",
          consolidator_prompt:
            "Consolida {{ ESTADO_ACTUAL }} con {{ BLOQUE_DE_MENSAJES }}",
          timeline_prompt: "Extrae hechos fechados",
        }),
      );
    });

    // El mensaje de éxito se muestra tras guardar.
    expect(await screen.findByText("Ajustes guardados")).toBeInTheDocument();
  });

  it("vuelve a mostrar un journal_prompt personalizado al reabrir (la siembra no lo pisa)", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    const first = renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Guardar un valor personalizado en la sub-pestaña Journal.
    await user.click(await screen.findByText("Prompts"));
    await user.click(screen.getByText("Journal"));

    const area = screen.getByLabelText("Journal Prompt");
    await user.clear(area);
    await user.type(area, "Mi relato personalizado");

    const form = area.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));
    await waitFor(() => expect(mockUpdateSettings).toHaveBeenCalled());
    first.unmount();

    // Reabrir el diálogo: el hook persiste lo guardado (el PUT mockeado
    // refresca `settings`) y el TextArea muestra el valor personalizado, no el
    // sembrado por la migración.
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);
    await user.click(await screen.findByText("Prompts"));
    await user.click(screen.getByText("Journal"));

    expect(screen.getByLabelText("Journal Prompt")).toHaveValue(
      "Mi relato personalizado",
    );
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

    await user.click(screen.getByText("Archivist"));
    const archivist = screen.getByLabelText("Archivist Prompt");
    await user.clear(archivist);
    await user.type(archivist, "Nuevo archivist");

    await user.click(screen.getByText("Collapse"));
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

  it("renders the five password fields and the two URL base fields in «Integraciones»", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // Cada clave se comprueba por su propio campo (etiqueta), nunca por un
    // conteo global de `input[type="password"]`: así el test no se rompe si
    // otra pestaña añade un `Input.Password`.
    expect(screen.getByLabelText("OpenWeatherMap API Key")).toHaveAttribute(
      "type",
      "password",
    );
    expect(screen.getByLabelText("Google Places API Key")).toHaveAttribute(
      "type",
      "password",
    );
    expect(screen.getByLabelText("Brave Search API Key")).toHaveAttribute(
      "type",
      "password",
    );
    expect(screen.getByLabelText("apimail · API Key")).toHaveAttribute(
      "type",
      "password",
    );
    expect(screen.getByLabelText("Miniflux · API Key")).toHaveAttribute(
      "type",
      "password",
    );

    // Los campos de URL base son de texto y, sin valor en settings, muestran
    // el placeholder con el endpoint por defecto.
    const apimailBaseUrl = screen.getByLabelText("apimail · URL base");
    expect(apimailBaseUrl).toHaveAttribute("type", "text");
    expect(apimailBaseUrl).toHaveAttribute(
      "placeholder",
      "https://apimail.territoriolinux.es",
    );

    const minifluxBaseUrl = screen.getByLabelText("Miniflux · URL base");
    expect(minifluxBaseUrl).toHaveAttribute("type", "text");
    expect(minifluxBaseUrl).toHaveAttribute(
      "placeholder",
      "https://miniflux.territoriolinux.es",
    );
  });

  it("renders the Miniflux URL base and API Key fields", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // La URL base de Miniflux es un campo de texto con el endpoint por defecto
    // como marcador; sin `miniflux_base_url` en settings no hay valor pintado.
    const baseUrl = screen.getByLabelText("Miniflux · URL base");
    expect(baseUrl).toHaveAttribute("type", "text");
    expect(baseUrl).toHaveAttribute(
      "placeholder",
      "https://miniflux.territoriolinux.es",
    );

    // El token de Miniflux es un campo de contraseña con el mismo marcador que
    // el resto de claves.
    const apiKey = screen.getByLabelText("Miniflux · API Key");
    expect(apiKey).toHaveAttribute("type", "password");
    expect(apiKey).toHaveAttribute(
      "placeholder",
      "Dejar vacío para usar variable de entorno",
    );
  });

  it("muestra la URL base vigente y guarda la configuración de Miniflux", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      miniflux_base_url: "https://miniflux.test.local",
      miniflux_api_key: "clave-previa",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    // Los campos de Miniflux viven en su propia sub-pestaña.
    await user.click(screen.getByRole("tab", { name: /^Miniflux(?![a-zA-Z])/ }));

    // El campo de URL base muestra el valor vigente.
    expect(screen.getByLabelText("Miniflux · URL base")).toHaveValue(
      "https://miniflux.test.local",
    );

    // La API key se edita y se envía al guardar.
    const apiKey = screen.getByLabelText("Miniflux · API Key");
    await user.clear(apiKey);
    await user.type(apiKey, "nueva-clave-miniflux");

    const form = apiKey.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          miniflux_api_key: "nueva-clave-miniflux",
          miniflux_base_url: "https://miniflux.test.local",
        }),
      );
    });
  });

  it("conserva la URL base de Miniflux al guardar desde otra pestaña", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      miniflux_base_url: "https://miniflux.test.local",
      miniflux_api_key: "clave",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Se abre «Integraciones» para que los campos de Miniflux queden
    // registrados en el formulario compartido.
    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // Se cambia a otra pestaña y se guarda desde ella: la URL base vigente no
    // debe perderse en el payload.
    await user.click(screen.getByText("Interfaz"));

    const fontSize = screen.getByLabelText("Tamaño de fuente");
    const form = fontSize.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          miniflux_base_url: "https://miniflux.test.local",
        }),
      );
    });
  });

  it("muestra la URL base vigente y guarda la configuración de apimail", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      apimail_base_url: "https://apimail.test.local",
      apimail_api_key: "clave-previa",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    // Los campos de apimail viven en su propia sub-pestaña.
    await user.click(screen.getByRole("tab", { name: /^apimail(?![a-zA-Z])/ }));

    // El campo de URL base muestra el valor vigente.
    expect(screen.getByLabelText("apimail · URL base")).toHaveValue(
      "https://apimail.test.local",
    );

    // La API key se edita y se envía al guardar.
    const apiKey = screen.getByLabelText("apimail · API Key");
    await user.clear(apiKey);
    await user.type(apiKey, "nueva-clave-apimail");

    const form = apiKey.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ apimail_api_key: "nueva-clave-apimail" }),
      );
    });
  });

  it("conserva la URL base de apimail al guardar desde otra pestaña", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      apimail_base_url: "https://apimail.test.local",
      apimail_api_key: "clave",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Se abre «Integraciones» para que los campos de apimail queden
    // registrados en el formulario compartido.
    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // Se cambia a otra pestaña y se guarda desde ella: la URL base vigente no
    // debe perderse en el payload.
    await user.click(screen.getByText("Interfaz"));

    const fontSize = screen.getByLabelText("Tamaño de fuente");
    const form = fontSize.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          apimail_base_url: "https://apimail.test.local",
        }),
      );
    });
  });

  // ════════════════════════════════════════════════════════════════
  // change `zennotes-notes` — token de ZenNotes en «Integraciones»
  // Escenarios:
  //  - «El token de ZenNotes nunca muestra el valor guardado»
  //  - «Guardar el token de ZenNotes usa su endpoint dedicado»
  // (openspec/changes/zennotes-notes/specs/frontend/spec.md)
  // ════════════════════════════════════════════════════════════════

  it("nunca muestra el valor guardado del token de ZenNotes y muestra «Configurado»", async () => {
    const user = userEvent.setup();
    // El estado sale de `GET /api/zennotes/status` (sin el token): configurado.
    mockGetZennotesStatus.mockResolvedValue({
      configured: true,
      base_url: "https://zennotes.territoriolinux.es",
      folder: "Valet",
    });
    // Aunque el token estuviera en settings, el campo NO debe leerlo de ahí.
    mockSettings = { ...mockSettings, zennotes_token: "SECRETO-no-mostrar" };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    // La sección vive en su propia sub-pestaña: se activa para que sus campos
    // y su botón estén accesibles (las panes inactivas quedan ocultas).
    await user.click(screen.getByRole("tab", { name: /^ZenNotes(?![a-zA-Z])/ }));

    // El campo existe y arranca vacío: no revela el valor guardado.
    const field = await screen.findByLabelText("ZenNotes · token de API");
    expect(field).toHaveValue("");
    // El estado se lee desde el endpoint dedicado, no desde settings. Se acota
    // a la sección porque el mismo texto viaja en el indicador de la etiqueta
    // de la sub-pestaña.
    expect(mockGetZennotesStatus).toHaveBeenCalled();
    expect(
      await within(screen.getByRole("region", { name: "ZenNotes" })).findByText(
        "Configurado",
      ),
    ).toBeInTheDocument();
  });

  it("guarda el token de ZenNotes por su endpoint dedicado y no por PUT /api/settings", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Las credenciales de integración viven ahora en «Integraciones» (una
    // sub-pestaña por servicio), no en la antigua pestaña «API Keys».
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await user.click(screen.getByRole("tab", { name: /^ZenNotes(?![a-zA-Z])/ }));

    const field = await screen.findByLabelText("ZenNotes · token de API");
    await user.type(field, "nuevo-token-zennotes");

    await user.click(screen.getByRole("button", { name: /guardar token/i }));

    // Se llama al endpoint dedicado con el token tecleado…
    await waitFor(() => {
      expect(mockSetZennotesToken).toHaveBeenCalledWith("nuevo-token-zennotes");
    });
    // …y NUNCA a `PUT /api/settings` (ni con `zennotes_token` ni sin él).
    expect(mockUpdateSettings).not.toHaveBeenCalled();

    // Tras guardar, el campo se limpia (no relee el valor guardado).
    await waitFor(() => {
      expect(screen.getByLabelText("ZenNotes · token de API")).toHaveValue("");
    });
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

  it("shows exactly ten top-level tabs and no Herramientas tab", () => {
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    const topTabs = screen.getAllByRole("tab");

    expect(topTabs).toHaveLength(10);
    for (const name of [
      "Perfil",
      "Interfaz",
      "Prompts",
      "Claves",
      "Memoria",
      "Generación",
      "Skills",
      "Enrutador de skills",
      "Integraciones",
      "Notificaciones",
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
          // change `chat-reasoning-default-off`: con el ajuste en `off`, el
          // formulario siembra `off` y ese valor viaja en el guardado.
          GENERATION_CHAT_REASONING: "off",
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
  // RED phase tests — change `chat-reasoning-default-off`
  // Escenario: "El razonamiento del chat aparece en `off` por defecto"
  // (openspec/changes/chat-reasoning-default-off/specs/frontend).
  // ════════════════════════════════════════════════════════════════

  it.each([
    {
      label: "la clave falta en GET /settings",
      value: undefined,
      expected: "off",
    },
    {
      label: "el valor llega vacío explícito",
      value: "",
      expected: "default",
    },
  ])(
    "muestra $expected en el razonamiento del chat cuando $label",
    async ({ value, expected }) => {
      const user = userEvent.setup();
      // Réplica de la respuesta real: la clave `GENERATION_CHAT_REASONING`
      // puede faltar (⇒ `off`) o venir vacía a propósito (⇒ `default`).
      const next: Record<string, string> = { ...defaultSettings };
      if (value === undefined) {
        delete next.GENERATION_CHAT_REASONING;
      } else {
        next.GENERATION_CHAT_REASONING = value;
      }
      mockSettings = next;
      renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

      await user.click(screen.getByText("Generación"));

      // El Select pinta el valor seleccionado en `.ant-select-selection-item`.
      const chatReasoning = screen.getByLabelText("GENERATION_CHAT_REASONING");
      const chatSelect = chatReasoning.closest(".ant-select") as HTMLElement;
      expect(within(chatSelect).getByText(expected)).toBeInTheDocument();
    },
  );

  it("un `default` explícito del chat se conserva: se muestra y se guarda vacío", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    // `GET /settings` devuelve vacío: la opción `default` elegida a propósito.
    mockSettings = { ...defaultSettings, GENERATION_CHAT_REASONING: "" };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    // El selector muestra `default`, no `off`.
    const chatReasoning = screen.getByLabelText("GENERATION_CHAT_REASONING");
    const chatSelect = chatReasoning.closest(".ant-select") as HTMLElement;
    expect(within(chatSelect).getByText("default")).toBeInTheDocument();

    // Guarda desde el bloque del chat sin editar el selector.
    const chatTemperature = screen.getByLabelText("GENERATION_CHAT_TEMPERATURE");
    const form = chatTemperature.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ GENERATION_CHAT_REASONING: "" }),
      );
    });
  });

  it("al guardar sin tocar el razonamiento del chat en off envía off", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    // El mock por defecto ya siembra `GENERATION_CHAT_REASONING: "off"`.
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    // Guarda desde el bloque del chat sin editar el selector de razonamiento.
    const chatTemperature = screen.getByLabelText("GENERATION_CHAT_TEMPERATURE");
    const form = chatTemperature.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ GENERATION_CHAT_REASONING: "off" }),
      );
    });
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

  it("shows the Línea temporal role with its three generation fields", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    expect(
      screen.getByRole("tab", { name: "Línea temporal" }),
    ).toBeInTheDocument();

    // Selecciona la sub-pestaña del timeline: el escenario de la spec exige
    // que sus tres claves queden visibles en el panel activo.
    await user.click(screen.getByRole("tab", { name: "Línea temporal" }));

    // Mismo patrón que el test vecino: la actividad de una sub-pestaña se lee
    // del `role="tabpanel"` (`aria-hidden="false"` activo, `"true"` oculto),
    // porque jsdom no emite `transitionend` y `toBeVisible()` no sirve. Con
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

    expect(screen.getByRole("tab", { name: "Línea temporal" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // Las tres claves del timeline quedan visibles en el panel activo.
    expectActive("GENERATION_TIMELINE_TEMPERATURE");
    expectActive("GENERATION_TIMELINE_REASONING");
    expectActive("GENERATION_TIMELINE_MAX_TOKENS");

    // Los paneles de los otros roles quedan ocultos.
    expectInactive("GENERATION_CHAT_TEMPERATURE");
    expectInactive("GENERATION_COLLAPSE_TEMPERATURE");
    expectInactive("GENERATION_SEMANTIC_TEMPERATURE");
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
  // El mecanismo real es el remontaje: el `Modal` de antd usa `destroyOnHidden`,
  // así que cada apertura vuelve a montar su contenido. Aquí se simula ese
  // remontaje cambiando la `key` del control, que es el comportamiento
  // observable equivalente: `RouterControl` remontado descarta la edición sin
  // guardar y re-lee `getSettings`.
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

  // ════════════════════════════════════════════════════════════════
  // change `react-tool-guards` — sección «Límites del agente»
  // Escenario: los cuatro umbrales de las guardas del bucle ReAct se
  // exponen en el diálogo y se persisten como cualquier ajuste.
  // ════════════════════════════════════════════════════════════════

  const AGENT_LIMIT_KEYS = [
    "REACT_TOOL_FINGERPRINT_LIMIT",
    "REACT_TOOL_FAILURE_LIMIT",
    "REACT_TOOL_SUCCESS_LIMIT",
    "REACT_TOOL_TOTAL_LIMIT",
  ] as const;

  it("renders the «Límites del agente» section with the four numeric fields", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    expect(screen.getByText("Límites del agente")).toBeInTheDocument();
    for (const key of AGENT_LIMIT_KEYS) {
      expect(screen.getByLabelText(key)).toBeInTheDocument();
    }
  });

  it("loads the agent limit values from the settings response", async () => {
    const user = userEvent.setup();
    mockSettings = {
      ...mockSettings,
      REACT_TOOL_FINGERPRINT_LIMIT: "5",
      REACT_TOOL_FAILURE_LIMIT: "7",
      REACT_TOOL_SUCCESS_LIMIT: "80",
      REACT_TOOL_TOTAL_LIMIT: "150",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    expect(screen.getByLabelText("REACT_TOOL_FINGERPRINT_LIMIT")).toHaveValue("5");
    expect(screen.getByLabelText("REACT_TOOL_FAILURE_LIMIT")).toHaveValue("7");
    expect(screen.getByLabelText("REACT_TOOL_SUCCESS_LIMIT")).toHaveValue("80");
    expect(screen.getByLabelText("REACT_TOOL_TOTAL_LIMIT")).toHaveValue("150");
  });

  it("saves the four agent limit keys via updateSettings", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    const fingerprint = screen.getByLabelText("REACT_TOOL_FINGERPRINT_LIMIT");
    await user.clear(fingerprint);
    await user.type(fingerprint, "4");

    const failure = screen.getByLabelText("REACT_TOOL_FAILURE_LIMIT");
    await user.clear(failure);
    await user.type(failure, "6");

    const success = screen.getByLabelText("REACT_TOOL_SUCCESS_LIMIT");
    await user.clear(success);
    await user.type(success, "60");

    const total = screen.getByLabelText("REACT_TOOL_TOTAL_LIMIT");
    await user.clear(total);
    await user.type(total, "120");

    const form = fingerprint.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          REACT_TOOL_FINGERPRINT_LIMIT: "4",
          REACT_TOOL_FAILURE_LIMIT: "6",
          REACT_TOOL_SUCCESS_LIMIT: "60",
          REACT_TOOL_TOTAL_LIMIT: "120",
        }),
      );
    });

    expect(await screen.findByText("Ajustes guardados")).toBeInTheDocument();
  });

  it("falls back to the compiled defaults when the agent limit keys are missing", async () => {
    const user = userEvent.setup();
    const next: Record<string, string> = { ...defaultSettings };
    for (const key of AGENT_LIMIT_KEYS) delete next[key];
    mockSettings = next;
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    expect(screen.getByLabelText("REACT_TOOL_FINGERPRINT_LIMIT")).toHaveValue("2");
    expect(screen.getByLabelText("REACT_TOOL_FAILURE_LIMIT")).toHaveValue("3");
    expect(screen.getByLabelText("REACT_TOOL_SUCCESS_LIMIT")).toHaveValue("50");
    expect(screen.getByLabelText("REACT_TOOL_TOTAL_LIMIT")).toHaveValue("100");
  });

  // `precision={0}` + `min={1}` ya impiden que se persista un literal inválido
  // al perder el foco (0 se ajusta a 1 y los decimales se redondean). La `rules`
  // de entero ≥ 1 es la red que rechaza el valor cuando se envía sin que el
  // campo pierda el foco (p. ej. con Intro): no se guarda y se muestra el error.
  it("rechaza un decimal en los límites del agente y no guarda", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    const field = screen.getByLabelText("REACT_TOOL_SUCCESS_LIMIT");
    await user.clear(field);
    await user.type(field, "2.5");

    // Se envía sin que el campo pierda el foco: la `rules` valida el valor
    // crudo (2.5) en vez del redondeado que aplicaría `precision` al hacer blur.
    const form = field.closest("form") as HTMLFormElement;
    fireEvent.submit(form);

    expect(await screen.findByText("Debe ser un entero ≥ 1")).toBeInTheDocument();
    expect(mockUpdateSettings).not.toHaveBeenCalled();
  });

  // El peligro real es persistir `0` (en el backend `successes >= 0` bloquea la
  // herramienta al instante). Con `min={1}` el `0` tecleado se ajusta a `1`: el
  // valor inválido nunca llega a guardarse.
  it.each([
    "REACT_TOOL_FINGERPRINT_LIMIT",
    "REACT_TOOL_FAILURE_LIMIT",
    "REACT_TOOL_SUCCESS_LIMIT",
    "REACT_TOOL_TOTAL_LIMIT",
  ])("no persiste «0» en %s: se ajusta al mínimo 1", async (key) => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    const field = screen.getByLabelText(key);
    await user.clear(field);
    await user.type(field, "0");

    const form = field.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalled();
    });
    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    expect(payload[key]).toBe("1");
    expect(payload[key]).not.toBe("0");
  });

  it("guarda un valor válido (entero ≥ 1) en los límites del agente", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByText("Generación"));

    const field = screen.getByLabelText("REACT_TOOL_SUCCESS_LIMIT");
    await user.clear(field);
    await user.type(field, "7");

    const form = field.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ REACT_TOOL_SUCCESS_LIMIT: "7" }),
      );
    });
  });

  // ════════════════════════════════════════════════════════════════
  // RED — change `settings-integrations-tabs`
  // Sub-pestañas horizontales en «Integraciones», indicador de configuración
  // por integración y pestaña «Claves» solo con claves de ingesta.
  // Escenarios: openspec/changes/settings-integrations-tabs/specs/frontend/spec.md
  // ════════════════════════════════════════════════════════════════

  // Orden cerrado de las sub-pestañas de «Integraciones» (spec aprobada). La
  // octava, «OpenTodo», la añade el change `opentodo-integration`.
  const INTEGRATION_SUBTABS = [
    "Strava",
    "OpenWeatherMap",
    "Google Places",
    "Brave Search",
    "apimail",
    "Miniflux",
    "ZenNotes",
    "OpenTodo",
  ] as const;

  // Etiquetas de los campos de credencial que, tras el change, viven SOLO en
  // «Integraciones» y nunca en «Claves».
  const CREDENTIAL_LABELS = [
    "OpenWeatherMap API Key",
    "Google Places API Key",
    "Brave Search API Key",
    "apimail · URL base",
    "apimail · API Key",
    "Miniflux · URL base",
    "Miniflux · API Key",
  ] as const;

  /** Abre la sub-pestaña de «Integraciones» cuyo nombre empieza por `name`. */
  const openIntegrationSubTab = async (
    user: ReturnType<typeof userEvent.setup>,
    name: string,
  ): Promise<HTMLElement> => {
    const tab = await screen.findByRole("tab", {
      name: new RegExp(`^${name}(?![a-zA-Z])`),
    });
    await user.click(tab);
    return tab;
  };

  it("organiza «Integraciones» en ocho sub-pestañas horizontales en el orden acordado", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // Ocho sub-pestañas, con «OpenTodo» como octava al final del orden cerrado.
    expect(INTEGRATION_SUBTABS).toHaveLength(8);
    expect(INTEGRATION_SUBTABS[7]).toBe("OpenTodo");

    const subTabs = INTEGRATION_SUBTABS.map((name) =>
      screen.getByRole("tab", {
        name: new RegExp(`^${name}(?![a-zA-Z])`),
      }),
    );

    // Las ocho cuelgan del mismo `tablist` anidado, distinto del de nivel
    // superior (que ya aloja Perfil, Interfaz, …).
    const nested = subTabs[0].closest('[role="tablist"]') as HTMLElement | null;
    if (!nested) throw new Error("Las sub-pestañas no cuelgan de un tablist");
    expect(nested).not.toBe(
      screen.getByRole("tab", { name: "Integraciones" }).closest('[role="tablist"]'),
    );
    for (const tab of subTabs) {
      expect(tab.closest('[role="tablist"]')).toBe(nested);
    }

    // Horizontales: antd no marca orientación vertical (no hay `tabPosition`).
    expect(nested).not.toHaveAttribute("aria-orientation", "vertical");

    // Orden: sus posiciones en el DOM son estrictamente crecientes.
    const domTabs = screen.getAllByRole("tab");
    const positions = subTabs.map((tab) => domTabs.indexOf(tab));
    expect(positions.every((index) => index >= 0)).toBe(true);
    expect(new Set(positions).size).toBe(INTEGRATION_SUBTABS.length);
    expect(positions).toEqual([...positions].sort((a, b) => a - b));

    // La sub-pestaña activa por defecto es la primera (Strava).
    expect(subTabs[0]).toHaveAttribute("aria-selected", "true");
  });

  it("cada sub-pestaña de clave muestra sus campos y su propio «Guardar»", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenWeatherMap");

    const field = await screen.findByLabelText("OpenWeatherMap API Key");
    // El panel activo no expone los campos de otras integraciones.
    const panel = field.closest('[role="tabpanel"]') as HTMLElement | null;
    expect(panel).not.toBeNull();
    expect(
      within(panel as HTMLElement).queryByLabelText("Brave Search API Key"),
    ).not.toBeInTheDocument();
    expect(
      within(panel as HTMLElement).queryByLabelText("apimail · URL base"),
    ).not.toBeInTheDocument();

    await user.type(field, "clave-openweather");

    // El «Guardar» de la sub-pestaña envía su propia clave.
    const form = field.closest("form") as HTMLFormElement;
    expect(form).not.toBeNull();
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ openweather_api_key: "clave-openweather" }),
      );
    });
  });

  it("apimail muestra la URL base vigente, enmascara la clave y guarda con su botón", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      apimail_base_url: "https://apimail.test.local",
      apimail_api_key: "clave-previa",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "apimail");

    expect(await screen.findByLabelText("apimail · URL base")).toHaveValue(
      "https://apimail.test.local",
    );

    const apiKey = screen.getByLabelText("apimail · API Key");
    expect(apiKey).toHaveAttribute("type", "password");

    await user.clear(apiKey);
    await user.type(apiKey, "nueva-clave-apimail");

    const form = apiKey.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ apimail_api_key: "nueva-clave-apimail" }),
      );
    });
  });

  it("las URL base conservan su placeholder por defecto y las claves el suyo", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    await openIntegrationSubTab(user, "apimail");
    expect(await screen.findByLabelText("apimail · URL base")).toHaveAttribute(
      "placeholder",
      "https://apimail.territoriolinux.es",
    );
    expect(screen.getByLabelText("apimail · API Key")).toHaveAttribute(
      "placeholder",
      "Dejar vacío para usar variable de entorno",
    );

    await openIntegrationSubTab(user, "Miniflux");
    expect(await screen.findByLabelText("Miniflux · URL base")).toHaveAttribute(
      "placeholder",
      "https://miniflux.territoriolinux.es",
    );

    await openIntegrationSubTab(user, "OpenWeatherMap");
    expect(
      await screen.findByLabelText("OpenWeatherMap API Key"),
    ).toHaveAttribute(
      "placeholder",
      "Dejar vacío para usar variable de entorno",
    );
  });

  it("Miniflux guarda con su propia sub-pestaña y no envía el token de ZenNotes", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      miniflux_base_url: "https://miniflux.test.local",
      miniflux_api_key: "clave-previa",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "Miniflux");

    const apiKey = await screen.findByLabelText("Miniflux · API Key");
    await user.clear(apiKey);
    await user.type(apiKey, "nueva-clave-miniflux");

    const form = apiKey.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ miniflux_api_key: "nueva-clave-miniflux" }),
      );
    });

    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    expect(payload).not.toHaveProperty("zennotes_token");
  });

  it("ZenNotes conserva su endpoint dedicado desde su sub-pestaña", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "ZenNotes");

    const field = await screen.findByLabelText("ZenNotes · token de API");
    await user.type(field, "nuevo-token-zennotes");
    await user.click(screen.getByRole("button", { name: /guardar token/i }));

    await waitFor(() => {
      expect(mockSetZennotesToken).toHaveBeenCalledWith("nuevo-token-zennotes");
    });
    // El token NUNCA viaja por `PUT /api/settings`.
    expect(mockUpdateSettings).not.toHaveBeenCalled();
  });

  // Los dos inputs de credenciales de Strava van con `component={false}`, así
  // que cuelgan del `<form>` real de ajustes del padre: sin interceptar Intro,
  // el navegador lanzaría el submit **implícito** de ese form
  // (`PUT /api/settings` sin credenciales), el diálogo se cerraría y las
  // credenciales no se guardarían.
  it("Intro en las credenciales de Strava guarda sus credenciales y no envía el formulario de ajustes", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={onClose} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    await user.type(await screen.findByLabelText("Strava Client ID"), "12345{enter}");

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledTimes(1);
    });
    // El payload es el de las credenciales, no el de los ajustes generales: si
    // hubiera entrado `handleSettingsSubmit`, llevaría `font_size` y compañía.
    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    expect(Object.keys(payload).sort()).toEqual([
      "strava_client_id",
      "strava_client_secret",
    ]);
    expect(payload.strava_client_id).toBe("12345");
    expect(payload).not.toHaveProperty("font_size");
    // Y el diálogo sigue abierto: el submit implícito ya no cierra nada.
    expect(onClose).not.toHaveBeenCalled();
  });

  it("Intro en el token de ZenNotes guarda por su endpoint y no envía el formulario de ajustes", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={onClose} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "ZenNotes");

    const field = await screen.findByLabelText("ZenNotes · token de API");
    await user.type(field, "token-con-intro{enter}");

    await waitFor(() => {
      expect(mockSetZennotesToken).toHaveBeenCalledWith("token-con-intro");
    });
    expect(mockSetZennotesToken).toHaveBeenCalledTimes(1);
    // Ni `PUT /api/settings` ni cierre del diálogo: una sola acción, no dos.
    expect(mockUpdateSettings).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("muestra el indicador de configuración en cada sub-pestaña desde el primer render", async () => {
    const user = userEvent.setup();
    // Strava conectada y ZenNotes configurado salen de sus endpoints dedicados.
    mockGetStravaStatus.mockResolvedValue({
      connected: true,
      athlete_id: "12345",
      athlete_name: "Ana Corredora",
      scope: "read,activity:read_all",
    });
    mockGetZennotesStatus.mockResolvedValue({
      configured: true,
      base_url: "https://zennotes.territoriolinux.es",
      folder: "Valet",
    });
    // Solo OpenWeatherMap tiene clave; apimail tiene URL base pero NO API key.
    mockSettings = {
      ...mockSettings,
      openweather_api_key: "clave-openweather",
      apimail_base_url: "https://apimail.test.local",
      apimail_api_key: "",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // Sin abrir ninguna sub-pestaña: los indicadores ya son correctos.
    expect(
      await screen.findByRole("tab", {
        name: /^Strava(?![a-zA-Z])[\s\S]*Conectada/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^OpenWeatherMap(?![a-zA-Z])[\s\S]*Configurada/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^Google Places(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^Brave Search(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();
    // apimail se evalúa por su API key: con URL base pero sin clave, «Sin configurar».
    expect(
      screen.getByRole("tab", {
        name: /^apimail(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^Miniflux(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^ZenNotes(?![a-zA-Z])[\s\S]*Configurado/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^OpenTodo(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();

    // Invariante de no duplicación: un solo `GET` de cada estado remoto por
    // apertura del panel, aunque el indicador y el panel compartan la lectura.
    expect(mockGetStravaStatus).toHaveBeenCalledTimes(1);
    expect(mockGetZennotesStatus).toHaveBeenCalledTimes(1);
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1);
  });

  // El nombre accesible del `Tabs` anidado evita que un lector de pantalla
  // anuncie siete pestañas sin decir de qué son (mismo patrón que el
  // `aria-label="Rol de generación"` del `Tabs` de «Generación»).
  //
  // antd propaga el `aria-label` al `div` raíz del `Tabs`, mientras el
  // `role="tablist"` vive en un hijo: etiquetar el atributo sin rol no crea un
  // nodo navegable con nombre. Por eso el panel añade `role="group"` y el test
  // aseverta sobre el **rol** (nombre accesible real), no sobre el atributo.
  it("da un nombre accesible navegable al «Tabs» anidado de Integraciones", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    const group = screen.getByRole("group", { name: "Integraciones" });
    // Las siete sub-pestañas viven dentro del grupo nombrado (no en el «Tabs»
    // de nivel superior).
    for (const name of INTEGRATION_SUBTABS) {
      expect(
        within(group).getByRole("tab", {
          name: new RegExp(`^${name}(?![a-zA-Z])`),
        }),
      ).toBeInTheDocument();
    }
  });

  // Los indicadores de Strava y ZenNotes se leen de sus endpoints al montar, y
  // el control del enrutador relee `GET /api/settings` al montar su pestaña.
  // La frescura no la da una señal de visibilidad, sino el ciclo de vida: el
  // `Modal` del diálogo lleva `destroyOnHidden`, así que al cerrar se desmonta
  // todo su contenido —`IntegrationsTab` y `RouterControl` incluidos— y al
  // abrir se vuelve a montar. Como los hooks leen en el montaje, cada apertura
  // gasta exactamente una lectura por endpoint, sin contadores, sin
  // `afterOpenChange` y sin depender de la animación del `Modal` para decidir
  // cuándo releer.
  it("relee cada endpoint en cada apertura porque el contenido se desmonta al cerrar", async () => {
    const user = userEvent.setup();
    const renderAt = (open: boolean) => (
      <ProfileProvider>
        <SettingsDialog visible={open} onClose={vi.fn()} />
      </ProfileProvider>
    );

    const { rerender } = renderDialog(renderAt(true));

    // Primera apertura: entrar en «Integraciones» monta el panel, cuyos hooks
    // hacen la única lectura de la apertura.
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await waitFor(() => {
      expect(mockGetStravaStatus).toHaveBeenCalledTimes(1);
    });
    expect(mockGetZennotesStatus).toHaveBeenCalledTimes(1);
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1);

    // Y entrar en «Enrutador de skills» monta su control, que relee
    // `GET /api/settings` por su cuenta: la cuenta de lecturas al terminar la
    // primera apertura queda registrada como línea base.
    await user.click(screen.getByRole("tab", { name: "Enrutador de skills" }));
    expect(
      await screen.findByRole("switch", { name: ROUTER_SWITCH }),
    ).toBeInTheDocument();
    const settingsReadsAfterFirstOpen = mockGetSettings.mock.calls.length;

    // Cerrar desmonta el contenido: el panel —y con él sus indicadores y el
    // control del enrutador— deja de existir en el DOM. Es la garantía
    // estructural del mecanismo: si el contenido sobreviviera al cierre, la
    // reapertura no remontaría los hooks y no habría segunda lectura.
    rerender(renderAt(false));
    await finishModalClose();
    expect(
      screen.queryByRole("group", { name: "Integraciones" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Strava")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("switch", { name: ROUTER_SWITCH }),
    ).not.toBeInTheDocument();
    expect(mockGetStravaStatus).toHaveBeenCalledTimes(1);
    expect(mockGetZennotesStatus).toHaveBeenCalledTimes(1);
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1);

    // Reabrir remonta el panel y añade exactamente una lectura más de cada
    // endpoint: una por apertura, ni acumulada por sesión ni duplicada.
    rerender(renderAt(true));
    await user.click(await screen.findByRole("tab", { name: "Integraciones" }));
    await waitFor(() => {
      expect(mockGetStravaStatus).toHaveBeenCalledTimes(2);
    });
    expect(mockGetZennotesStatus).toHaveBeenCalledTimes(2);
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(2);

    // La pestaña del enrutador también se remonta de cero y su control relee
    // lo persistido en esta apertura: la cuenta de `GET /api/settings` supera
    // la línea base de la primera apertura.
    await user.click(
      await screen.findByRole("tab", { name: "Enrutador de skills" }),
    );
    expect(
      await screen.findByRole("switch", { name: ROUTER_SWITCH }),
    ).toBeInTheDocument();
    expect(mockGetSettings.mock.calls.length).toBeGreaterThan(
      settingsReadsAfterFirstOpen,
    );
  });

  // Un fallo de lectura no puede dejar el indicador en «Consultando…» para
  // siempre: eso afirmaría que se está consultando cuando ya no, y ocultaría
  // que el estado es desconocido.
  it("muestra «Sin datos» cuando falla la lectura del estado de Strava o ZenNotes", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus.mockRejectedValue(new Error("Strava caído"));
    mockGetZennotesStatus.mockRejectedValue(new Error("ZenNotes caído"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    expect(
      await screen.findByRole("tab", {
        name: /^Strava(?![a-zA-Z])[\s\S]*Sin datos/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", {
        name: /^ZenNotes(?![a-zA-Z])[\s\S]*Sin datos/,
      }),
    ).toBeInTheDocument();
    // El texto de pendiente desaparece: no hay consulta en curso.
    expect(screen.queryByText("Consultando…")).not.toBeInTheDocument();
  });

  // La spec fija «Configurado»/«Sin configurar»: el panel y la etiqueta no
  // pueden decir cosas distintas para el mismo estado.
  it("usa «Sin configurar» en el panel de ZenNotes, igual que en la etiqueta", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "ZenNotes");

    const section = await screen.findByRole("region", { name: "ZenNotes" });
    expect(
      await within(section).findByText("Sin configurar"),
    ).toBeInTheDocument();
    expect(within(section).queryByText("No configurado")).not.toBeInTheDocument();
  });

  it("marca «No conectada» la sub-pestaña de Strava cuando no hay conexión", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus.mockResolvedValue({
      connected: false,
      athlete_id: null,
      athlete_name: null,
      scope: null,
    });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    expect(
      await screen.findByRole("tab", {
        name: /^Strava(?![a-zA-Z])[\s\S]*No conectada/,
      }),
    ).toBeInTheDocument();
  });

  it("actualiza el indicador a «Configurada» tras guardar la clave de su sub-pestaña", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    // De partida, sin `google_places_api_key`, la sub-pestaña está sin configurar.
    expect(
      await screen.findByRole("tab", {
        name: /^Google Places(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();

    await openIntegrationSubTab(user, "Google Places");
    const field = await screen.findByLabelText("Google Places API Key");
    await user.type(field, "clave-google-places");

    const form = field.closest("form") as HTMLFormElement;
    await user.click(within(form).getByRole("button", { name: /guardar/i }));

    // Sin recargar la página, el indicador de la etiqueta refleja el guardado.
    await waitFor(() => {
      expect(
        screen.getByRole("tab", {
          name: /^Google Places(?![a-zA-Z])[\s\S]*Configurada/,
        }),
      ).toBeInTheDocument();
    });
  });

  it("el estado de cada sub-pestaña se anuncia con texto accesible, no solo con color", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus.mockResolvedValue({
      connected: true,
      athlete_id: "12345",
      athlete_name: "Ana Corredora",
      scope: "read,activity:read_all",
    });
    mockGetZennotesStatus.mockResolvedValue({
      configured: true,
      base_url: "https://zennotes.territoriolinux.es",
      folder: "Valet",
    });
    mockGetOpentodoStatus.mockResolvedValue({
      configured: true,
      base_url: "https://todo.territoriolinux.es",
    });
    mockSettings = {
      ...mockSettings,
      openweather_api_key: "clave-openweather",
      google_places_api_key: "clave-places",
      brave_search_api_key: "clave-brave",
      apimail_base_url: "https://apimail.test.local",
      apimail_api_key: "clave-apimail",
      miniflux_base_url: "https://miniflux.test.local",
      miniflux_api_key: "clave-miniflux",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    // El nombre accesible de la etiqueta lleva el estado en texto: un lector de
    // pantalla lo anuncia sin depender del color del indicador.
    const expected: Array<[string, RegExp]> = [
      ["Strava", /Conectada/],
      ["OpenWeatherMap", /Configurada/],
      ["Google Places", /Configurada/],
      ["Brave Search", /Configurada/],
      ["apimail", /Configurada/],
      ["Miniflux", /Configurada/],
      ["ZenNotes", /Configurado/],
      ["OpenTodo", /Configurado/],
    ];
    for (const [name, state] of expected) {
      const tab = await screen.findByRole("tab", {
        name: new RegExp(`^${name}(?![a-zA-Z])`),
      });
      expect(tab).toHaveAccessibleName(state);
    }
  });

  it("la pestaña «Claves» contiene solo la gestión de claves de ingesta", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Claves" }));

    // Gestión de claves de ingesta: alta y listado con prefijo y fecha de uso.
    const section = await screen.findByRole("region", {
      name: "Claves de ingesta",
    });
    expect(
      within(section).getByLabelText("Nombre de la aplicación"),
    ).toBeInTheDocument();
    expect(
      within(section).getByRole("button", { name: /crear clave/i }),
    ).toBeInTheDocument();
    expect(
      within(section).getByRole("columnheader", { name: "Prefijo" }),
    ).toBeInTheDocument();
    expect(
      within(section).getByRole("columnheader", { name: "Último uso" }),
    ).toBeInTheDocument();

    // Ninguna credencial de integración vive ya en «Claves».
    for (const label of CREDENTIAL_LABELS) {
      expect(screen.queryByLabelText(label)).not.toBeInTheDocument();
    }
    expect(
      screen.queryByLabelText("ZenNotes · token de API"),
    ).not.toBeInTheDocument();
  });

  // ════════════════════════════════════════════════════════════════
  // RED — change `opentodo-integration`
  // Octava sub-pestaña «OpenTodo» de «Integraciones»: URL base por el
  // formulario compartido, token por su endpoint dedicado e indicador desde
  // `GET /api/opentodo/status`.
  // Escenarios: openspec/changes/opentodo-integration/specs/frontend/spec.md
  // ════════════════════════════════════════════════════════════════

  it("el panel de OpenTodo tiene URL base, token enmascarado, estado y «Probar conexión»", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const region = await screen.findByRole("region", { name: "OpenTodo" });

    // Campo de URL base con su placeholder por defecto.
    const baseUrl = within(region).getByLabelText("OpenTodo · URL base");
    expect(baseUrl).toHaveAttribute("type", "text");
    expect(baseUrl).toHaveAttribute(
      "placeholder",
      "https://todo.territoriolinux.es",
    );

    // Campo de token enmascarado.
    expect(within(region).getByLabelText("OpenTodo · token de API")).toHaveAttribute(
      "type",
      "password",
    );

    // Línea de estado y botón «Probar conexión».
    expect(within(region).getByText("Estado")).toBeInTheDocument();
    expect(
      within(region).getByRole("button", { name: "Probar conexión" }),
    ).toBeInTheDocument();
    // Y su propio botón «Guardar» (submit del formulario de ajustes).
    expect(
      within(region).getByRole("button", { name: "Guardar" }),
    ).toBeInTheDocument();
  });

  it("el campo del token de OpenTodo arranca vacío aunque haya token guardado", async () => {
    const user = userEvent.setup();
    // El estado sale de `GET /api/opentodo/status` (sin el token): configurado.
    mockGetOpentodoStatus.mockResolvedValue({
      configured: true,
      base_url: "https://todo.territoriolinux.es",
    });
    // Aunque el token estuviera en settings, el campo NO debe leerlo de ahí.
    mockSettings = { ...mockSettings, opentodo_token: "SECRETO-no-mostrar" };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const field = await screen.findByLabelText("OpenTodo · token de API");
    expect(field).toHaveValue("");

    // El estado «Configurado» se muestra en la línea de estado, no en el campo.
    const region = screen.getByRole("region", { name: "OpenTodo" });
    expect(await within(region).findByText("Configurado")).toBeInTheDocument();
    expect(mockGetOpentodoStatus).toHaveBeenCalled();
  });

  it("guarda el token de OpenTodo por su endpoint dedicado y no por PUT /api/settings", async () => {
    const user = userEvent.setup();
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const field = await screen.findByLabelText("OpenTodo · token de API");
    await user.type(field, "nuevo-token-opentodo");

    await user.click(screen.getByRole("button", { name: /guardar token/i }));

    // Se llama al endpoint dedicado con el token tecleado…
    await waitFor(() => {
      expect(mockSetOpentodoToken).toHaveBeenCalledWith("nuevo-token-opentodo");
    });
    // …y NUNCA a `PUT /api/settings`.
    expect(mockUpdateSettings).not.toHaveBeenCalled();

    // Tras guardar, el campo se limpia (no relee el valor guardado).
    await waitFor(() => {
      expect(screen.getByLabelText("OpenTodo · token de API")).toHaveValue("");
    });
  });

  it("guarda la URL base de OpenTodo con el formulario de ajustes", async () => {
    const user = userEvent.setup();
    mockUpdateSettings.mockResolvedValue(undefined);
    mockSettings = {
      ...mockSettings,
      opentodo_base_url: "https://todo.test.local",
    };
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    // El campo de URL base muestra el valor vigente.
    const baseUrl = await screen.findByLabelText("OpenTodo · URL base");
    expect(baseUrl).toHaveValue("https://todo.test.local");

    await user.clear(baseUrl);
    await user.type(baseUrl, "https://todo.nuevo.local");

    // El «Guardar» de la sub-pestaña es el submit del formulario compartido.
    const region = screen.getByRole("region", { name: "OpenTodo" });
    await user.click(within(region).getByRole("button", { name: "Guardar" }));

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ opentodo_base_url: "https://todo.nuevo.local" }),
      );
    });
    // El token nunca viaja por `PUT /api/settings`.
    const payload = mockUpdateSettings.mock.calls[0][0] as Record<string, string>;
    expect(payload).not.toHaveProperty("opentodo_token");
  });

  it("el indicador de OpenTodo sale de su endpoint y es visible sin abrir la sub-pestaña", async () => {
    const user = userEvent.setup();
    mockGetOpentodoStatus.mockResolvedValue({
      configured: true,
      base_url: "https://todo.territoriolinux.es",
    });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    // Sin abrir la sub-pestaña «OpenTodo»: el indicador ya es correcto.
    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    expect(
      await screen.findByRole("tab", {
        name: /^OpenTodo(?![a-zA-Z])[\s\S]*Configurado/,
      }),
    ).toBeInTheDocument();
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1);
  });

  it("tras guardar el token, el indicador de OpenTodo pasa a «Configurado» sin recargar", async () => {
    const user = userEvent.setup();
    // Primera lectura (montaje): sin configurar. Segunda (tras guardar el
    // token): configurado. Refleja el ciclo real del endpoint.
    mockGetOpentodoStatus
      .mockResolvedValueOnce({
        configured: false,
        base_url: "https://todo.territoriolinux.es",
      })
      .mockResolvedValueOnce({
        configured: true,
        base_url: "https://todo.territoriolinux.es",
      });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    expect(
      await screen.findByRole("tab", {
        name: /^OpenTodo(?![a-zA-Z])[\s\S]*Sin configurar/,
      }),
    ).toBeInTheDocument();

    await openIntegrationSubTab(user, "OpenTodo");
    const field = await screen.findByLabelText("OpenTodo · token de API");
    await user.type(field, "token-nuevo");
    await user.click(screen.getByRole("button", { name: /guardar token/i }));

    await waitFor(() => {
      expect(mockSetOpentodoToken).toHaveBeenCalledWith("token-nuevo");
    });
    // Sin recargar la página, el indicador de la etiqueta refleja el guardado.
    expect(
      await screen.findByRole("tab", {
        name: /^OpenTodo(?![a-zA-Z])[\s\S]*Configurado/,
      }),
    ).toBeInTheDocument();
  });

  // Un fallo de lectura no puede dejar el panel diciendo «Sin configurar»
  // mientras la etiqueta dice «Sin datos»: ambos deben usar la misma etiqueta
  // compartida de `integrationStatus.ts`.
  it("usa «Sin datos» en la etiqueta y en el panel de OpenTodo cuando falla la lectura", async () => {
    const user = userEvent.setup();
    mockGetOpentodoStatus.mockRejectedValue(new Error("OpenTodo caído"));
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));

    expect(
      await screen.findByRole("tab", {
        name: /^OpenTodo(?![a-zA-Z])[\s\S]*Sin datos/,
      }),
    ).toBeInTheDocument();

    await openIntegrationSubTab(user, "OpenTodo");
    const region = await screen.findByRole("region", { name: "OpenTodo" });
    // El panel no puede contradecir a la etiqueta: dice «Sin datos», no
    // «Sin configurar».
    expect(await within(region).findByText("Sin datos")).toBeInTheDocument();
    expect(within(region).queryByText("Sin configurar")).not.toBeInTheDocument();
  });

  it("«Probar conexión» muestra el detalle del sondeo fallido en la línea de estado", async () => {
    const user = userEvent.setup();
    mockCheckOpentodo.mockResolvedValue({
      ok: false,
      detail: "el token no es válido o ha caducado",
    });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const region = await screen.findByRole("region", { name: "OpenTodo" });
    await user.click(within(region).getByRole("button", { name: "Probar conexión" }));

    // El resultado de la acción vive en la línea de estado (`aria-live`), no
    // solo en el toast.
    expect(
      await within(region).findByText("el token no es válido o ha caducado"),
    ).toBeInTheDocument();
  });

  it("«Probar conexión» muestra el texto de éxito en la línea de estado", async () => {
    const user = userEvent.setup();
    mockCheckOpentodo.mockResolvedValue({
      ok: true,
      detail: "conexión correcta",
    });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const region = await screen.findByRole("region", { name: "OpenTodo" });
    await user.click(within(region).getByRole("button", { name: "Probar conexión" }));

    expect(
      await within(region).findByText("conexión correcta"),
    ).toBeInTheDocument();
  });

  it("«Probar conexión» usa GET /api/opentodo/check y no relee el estado", async () => {
    const user = userEvent.setup();
    mockCheckOpentodo.mockResolvedValue({
      ok: true,
      detail: "conexión correcta",
    });
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const region = await screen.findByRole("region", { name: "OpenTodo" });
    await waitFor(() => expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1));
    const statusReadsBeforeCheck = mockGetOpentodoStatus.mock.calls.length;

    await user.click(within(region).getByRole("button", { name: "Probar conexión" }));

    await waitFor(() => expect(mockCheckOpentodo).toHaveBeenCalledTimes(1));
    // El sondeo no toca `GET /api/opentodo/status`.
    expect(mockGetOpentodoStatus.mock.calls.length).toBe(statusReadsBeforeCheck);
  });

  // El botón refleja el sondeo **en curso**, no solo su resultado: con la
  // promesa de `GET /api/opentodo/check` pendiente se declara ocupado
  // (`aria-busy="true"`) y en estado de carga (`ant-btn-loading`), y vuelve a
  // `false` en cuanto la respuesta llega con su `detail` a la línea de estado.
  // Cubre la ventana que un usuario real ve mientras la instancia remota tarda
  // en contestar.
  it("«Probar conexión» marca el botón como ocupado mientras el sondeo está pendiente", async () => {
    const user = userEvent.setup();
    let resolveCheck!: (result: OpentodoCheckResult) => void;
    // El sondeo queda a la espera: la promesa no se resuelve hasta que el test
    // la libere.
    mockCheckOpentodo.mockReturnValue(
      new Promise<OpentodoCheckResult>((resolve) => {
        resolveCheck = resolve;
      }),
    );
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const region = await screen.findByRole("region", { name: "OpenTodo" });
    const button = within(region).getByRole("button", {
      name: "Probar conexión",
    });

    // Punto de partida: sin sondeo en vuelo, el botón no está ocupado.
    expect(button).toHaveAttribute("aria-busy", "false");
    expect(button).not.toHaveClass("ant-btn-loading");

    await user.click(button);

    // Promesa pendiente ⇒ ocupado y con el indicador de carga de antd.
    await waitFor(() => {
      expect(button).toHaveAttribute("aria-busy", "true");
    });
    expect(button).toHaveClass("ant-btn-loading");

    // Se resuelve el sondeo dentro de `act`: el `setState` con el resultado
    // forma parte del ciclo de React, no de un microtask suelto.
    await act(async () => {
      resolveCheck({ ok: true, detail: "conexión correcta" });
    });

    // El `detail` aterriza en la línea de estado y el botón se libera.
    expect(
      await within(region).findByText("conexión correcta"),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(button).toHaveAttribute("aria-busy", "false");
    });
    expect(button).not.toHaveClass("ant-btn-loading");
  });

  // El «Guardar» del panel es el submit del formulario compartido
  // (`PUT /api/settings`), así que su estado ocupado lo dicta `saving`, que
  // viaja `SettingsDialog` → `IntegrationsTab` → `OpentodoSection`: si ese
  // camino se rompiera, el botón invitaría a un doble guardado.
  it("el «Guardar» de OpenTodo se declara ocupado mientras `saving` está activo", async () => {
    const user = userEvent.setup();
    mockSaving = true;
    renderDialog(<ProfileProvider><SettingsDialog visible={true} onClose={vi.fn()} /></ProfileProvider>);

    await user.click(screen.getByRole("tab", { name: "Integraciones" }));
    await openIntegrationSubTab(user, "OpenTodo");

    const region = await screen.findByRole("region", { name: "OpenTodo" });
    // Con `loading` activo, antd antepone al nombre accesible el del icono de
    // carga (`loading`), así que el botón se anuncia como «loadingGuardar».
    // El patrón acota al submit «Guardar» sin casar con «Guardar token», que
    // sigue libre (no lo gobierna `saving`).
    const saveButton = within(region).getByRole("button", {
      name: /Guardar$/,
    });

    expect(saveButton).toHaveAttribute("aria-busy", "true");
    await waitFor(() => {
      expect(saveButton).toHaveClass("ant-btn-loading");
    });

    // El «Guardar token» de su endpoint dedicado es independiente: no lo
    // gobierna `saving`, sino el guardado del propio token.
    expect(
      within(region).getByRole("button", { name: /guardar token/i }),
    ).toHaveAttribute("aria-busy", "false");
  });
});
