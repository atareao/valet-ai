import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { AppLayout } from "../components/AppLayout";
import { ProfileProvider } from "../contexts/ProfileProvider";

// Arneses mínimos: se mockean los hijos con dependencias complejas y se deja
// que el botón/modal de avisos sea el objeto de la prueba. `NotificationsView`
// se mockea (el stub real basta, pero así el lazy resuelve sin Suspense async).
vi.mock("../pages/StatsDashboard", () => ({
  StatsDashboard: () => <div data-testid="stats-dashboard">Stats Content</div>,
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">Chat</div>,
}));

vi.mock("../components/SettingsDialog", () => ({
  SettingsDialog: () => null,
}));

vi.mock("../components/CalendarView", () => ({
  CalendarView: () => <div data-testid="calendar-view">Calendar</div>,
}));

vi.mock("../components/NotificationsView", () => ({
  NotificationsView: () => (
    <div data-testid="notifications-view">Notifications</div>
  ),
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

describe("AppLayout — avisos", () => {
  it("shows a bell button in the header", () => {
    renderLayout();

    expect(screen.getByRole("button", { name: /avisos/i })).toBeInTheDocument();
  });

  it("opens the notifications dialog when the bell is clicked", async () => {
    const user = userEvent.setup();
    renderLayout();

    expect(screen.queryByText("🔔 Avisos")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /avisos/i }));

    await waitFor(() => {
      expect(screen.getByText("🔔 Avisos")).toBeInTheDocument();
    });

    expect(screen.getByTestId("notifications-view")).toBeInTheDocument();
  });
});
