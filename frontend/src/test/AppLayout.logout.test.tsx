import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppLayout } from "../components/AppLayout";
import { AuthProvider } from "../contexts/AuthProvider";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Contrato (spec `frontend`, scenario "Logout local y SSO desde el header"):
 * el botón de logout llama a `POST /api/auth/logout` y redirige al
 * `end_session_url` devuelto por el backend (`src/routes/auth.rs` →
 * `LogoutResponse`). El mock usa el nombre real del campo para que el test
 * falle si el frontend vuelve a divergir del contrato.
 */
vi.mock("../hooks/useMainChat", () => ({
  useMainChat: () => ({
    messages: [],
    loading: false,
    sendMessage: vi.fn(),
    streaming: false,
    streamingContent: "",
    activeTools: [],
    pendingApproval: null,
    resolveApproval: vi.fn(),
    widgetsByMessage: {},
    sendWidgetAction: vi.fn(),
  }),
}));

vi.mock("../hooks/useSettings", () => ({
  useSettings: () => ({ settings: {}, loading: false, error: null }),
}));

vi.mock("../contexts/ProfileContext", () => ({
  useProfileContext: () => ({
    profile: null,
    loading: false,
    error: null,
    updateProfile: vi.fn(),
  }),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view" />,
}));
vi.mock("../components/SettingsDialog", () => ({
  SettingsDialog: () => <div data-testid="settings-dialog" />,
}));
vi.mock("../components/CalendarView", () => ({
  CalendarView: () => <div data-testid="calendar-view" />,
}));
vi.mock("../components/TaskView", () => ({
  TaskView: () => <div data-testid="task-view" />,
}));
vi.mock("../pages/StatsDashboard", () => ({
  StatsDashboard: () => <div data-testid="stats-dashboard" />,
}));

const originalLocation = window.location;

function stubLocation() {
  const location = {
    href: "",
    assign: vi.fn((url: string) => {
      location.href = url;
    }),
    replace: vi.fn((url: string) => {
      location.href = url;
    }),
  };
  Object.defineProperty(window, "location", {
    configurable: true,
    writable: true,
    value: location,
  });
  return location;
}

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

afterEach(() => {
  vi.unstubAllGlobals();
  Object.defineProperty(window, "location", {
    configurable: true,
    writable: true,
    value: originalLocation,
  });
});

describe("AppLayout — logout", () => {
  it("llama a POST /api/auth/logout y redirige al end_session_url", async () => {
    const location = stubLocation();
    const user = userEvent.setup();

    const fetchMock = vi.fn(
      async (url: RequestInfo | URL, _init?: RequestInit): Promise<Response> => {
        const target = String(url);
        if (target.includes("/api/auth/me")) {
          return {
            ok: true,
            status: 200,
            json: async () => ({ sub: "u1", email: "a@b.c", name: "Lorenzo" }),
          } as unknown as Response;
        }
        if (target.includes("/api/auth/logout")) {
          return {
            ok: true,
            status: 200,
            json: async () => ({
              end_session_url: "https://idp.example/logout",
            }),
          } as unknown as Response;
        }
        return {
          ok: true,
          status: 200,
          json: async () => ({}),
        } as unknown as Response;
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    render(
      <AuthProvider>
        <AppLayout />
      </AuthProvider>,
    );

    const logoutButton = await screen.findByRole("button", {
      name: /cerrar sesión|logout|salir/i,
    });
    await user.click(logoutButton);

    const logoutCall = fetchMock.mock.calls.find(([url]) =>
      String(url).includes("/api/auth/logout"),
    );
    expect(logoutCall?.[1]?.method).toBe("POST");

    await waitFor(() =>
      expect(location.href).toBe("https://idp.example/logout"),
    );
  });
});
