import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { AppLayout } from "./AppLayout";
import { ProfileProvider } from "../contexts/ProfileProvider";
// El fuente tal cual, para comprobar cómo se carga la vista de Notas: no hay
// forma de observar desde jsdom si un chunk entra o no en el bundle inicial (el
// fuente de `AppLayout` se lee con la API `?raw` de Vite, que declara su tipo en
// `vite/client`).
import appLayoutSource from "./AppLayout.tsx?raw";

// Arneses mínimos: se mockean los hijos con dependencias complejas y se deja
// que el botón/modal de Notas sea el objeto de la prueba. `NotesView` se
// mockea (el stub basta, pero así el lazy resuelve sin Suspense asíncrono).
vi.mock("../pages/StatsDashboard", () => ({
  StatsDashboard: () => <div data-testid="stats-dashboard">Stats Content</div>,
}));

vi.mock("./ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">Chat</div>,
}));

vi.mock("./SettingsDialog", () => ({
  SettingsDialog: () => null,
}));

vi.mock("./CalendarView", () => ({
  CalendarView: () => <div data-testid="calendar-view">Calendar</div>,
}));

vi.mock("./TaskView", () => ({
  TaskView: () => <div data-testid="task-view">Tasks</div>,
}));

vi.mock("./NotificationsView", () => ({
  NotificationsView: () => (
    <div data-testid="notifications-view">Notifications</div>
  ),
}));

vi.mock("./JournalView", () => ({
  JournalView: () => <div data-testid="journal-view">Journal</div>,
}));

vi.mock("./NotesView", () => ({
  NotesView: () => <div data-testid="notes-view">Notes</div>,
}));

vi.mock("../hooks/useMainChat", () => ({
  useMainChat: () => ({
    messages: [],
    loading: false,
    sendMessage: vi.fn(),
    streaming: false,
    streamingContent: null,
    activeTools: [],
    pendingApproval: null,
    resolveApproval: vi.fn(),
    widgetsByMessage: {},
    sendWidgetAction: vi.fn(),
  }),
}));

vi.mock("../hooks/useProfile", () => ({
  useProfile: () => ({
    profile: { id: "test", name: "Test", avatar_url: "", preferences: "{}" },
    loading: false,
    error: null,
    updateProfile: vi.fn(),
  }),
}));

vi.mock("../hooks/useSettings", () => ({
  useSettings: () => ({
    settings: {},
    loading: false,
    saving: false,
    error: null,
    updateSettings: vi.fn(),
    resetToDefaults: vi.fn(),
  }),
}));

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

const renderLayout = () =>
  render(
    <MemoryRouter>
      <ProfileProvider>
        <AppLayout />
      </ProfileProvider>
    </MemoryRouter>,
  );

describe("AppLayout — notas", () => {
  it("muestra la entrada de Notas en el header", () => {
    renderLayout();

    expect(screen.getByRole("button", { name: /notas/i })).toBeInTheDocument();
  });

  it("abre la vista de Notas al pulsar la entrada del header", async () => {
    const user = userEvent.setup();
    renderLayout();

    await user.click(screen.getByRole("button", { name: /notas/i }));

    await waitFor(() => {
      expect(screen.getByText("📝 Notas")).toBeInTheDocument();
    });
    expect(screen.getByTestId("notes-view")).toBeInTheDocument();
  });

  it("la vista no se monta hasta que se abre su modal", async () => {
    const user = userEvent.setup();
    renderLayout();

    expect(screen.queryByTestId("notes-view")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /notas/i }));

    await waitFor(() => {
      expect(screen.getByTestId("notes-view")).toBeInTheDocument();
    });
  });

  it("carga la vista bajo React.lazy + Suspense, no en el bundle inicial", () => {
    // Sin import estático...
    expect(appLayoutSource).not.toContain('import { NotesView }');
    // ...sino diferido, y renderizado dentro de un `Suspense`.
    expect(appLayoutSource).toMatch(
      /lazy\(\(\)\s*=>\s*import\("\.\/NotesView"\)/,
    );
    expect(appLayoutSource).toContain("Suspense fallback={lazyViewFallback}");
  });
});
