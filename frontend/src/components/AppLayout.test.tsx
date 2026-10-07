import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { AppLayout } from "./AppLayout";
import { ProfileProvider } from "../contexts/ProfileProvider";

// Mock child components that have complex dependencies
vi.mock("../pages/StatsDashboard", () => ({
  StatsDashboard: () => <div data-testid="stats-dashboard">Stats Content</div>,
}));

vi.mock("./ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">Chat</div>,
}));

vi.mock("./SettingsDialog", () => ({
  SettingsDialog: ({ visible, onClose }: { visible: boolean; onClose: () => void }) =>
    visible ? (
      <div data-testid="settings-dialog">
        <span>⚙️ Settings</span>
        <button onClick={onClose}>Close</button>
      </div>
    ) : null,
}));

vi.mock("./CalendarView", () => ({
  CalendarView: () => <div data-testid="calendar-view">Calendar</div>,
}));

vi.mock("./TaskView", () => ({
  TaskView: () => <div data-testid="task-view">Tasks</div>,
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

describe("AppLayout — settings dialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("settings modal is closed initially", () => {
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("settings-dialog")).not.toBeInTheDocument();
  });

  it("opens settings modal when clicking the gear icon", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    const settingsButton = screen.getByRole("button", { name: /setting/i });
    expect(settingsButton).toBeInTheDocument();

    await user.click(settingsButton);
    await waitFor(() => {
      expect(screen.getByTestId("settings-dialog")).toBeInTheDocument();
    });
  });

  it("has only one settings-related button in the header (no profile button)", () => {
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    // There should be a button with SettingOutlined icon
    expect(screen.getByRole("button", { name: /setting/i })).toBeInTheDocument();
    // There should NOT be a separate user/profile button anymore
    expect(screen.queryByRole("button", { name: /user/i })).not.toBeInTheDocument();
  });

  it("stats modal is closed initially", () => {
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("stats-dashboard")).not.toBeInTheDocument();
  });

  it("opens stats modal when clicking the bar chart icon", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    const statsButton = screen.getByRole("button", { name: /bar-chart/i });
    expect(statsButton).toBeInTheDocument();

    await user.click(statsButton);
    await waitFor(() => {
      expect(screen.getByTestId("stats-dashboard")).toBeInTheDocument();
    });
  });

  it("calendar view is not mounted until its modal opens", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("calendar-view")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /calendar/i }));
    await waitFor(() => {
      expect(screen.getByTestId("calendar-view")).toBeInTheDocument();
    });
  });

  it("task view is not mounted until its modal opens", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <ProfileProvider>
          <AppLayout />
        </ProfileProvider>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("task-view")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /check-square/i }));
    await waitFor(() => {
      expect(screen.getByTestId("task-view")).toBeInTheDocument();
    });
  });
});