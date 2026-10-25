import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChatView } from "../components/ChatView";
import type { Message } from "../types";

// ---------------------------------------------------------------------------
// Ant Design matchMedia mock (mismo patrón que SettingsDialog.test.tsx)
// ---------------------------------------------------------------------------
beforeEach(() => {
  // jsdom no implementa scrollIntoView; ChatView lo usa en un useEffect.
  window.HTMLElement.prototype.scrollIntoView = vi.fn();

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

const userMessage: Message = {
  id: "u1",
  role: "user",
  content: "Hola",
  created_at: "2026-09-27T10:30:00Z",
};

interface ChatViewPropsOverrides {
  messages?: Message[];
  userAvatarUrl?: string | null;
  streaming?: boolean;
  streamingContent?: string;
  activeTools?: string[];
}

function renderChatView(overrides: ChatViewPropsOverrides = {}) {
  const {
    messages = [userMessage],
    userAvatarUrl,
    streaming = false,
    streamingContent = "",
    activeTools = [],
  } = overrides;

  return render(
    <ChatView
      messages={messages}
      loading={false}
      onSendMessage={vi.fn()}
      streaming={streaming}
      streamingContent={streamingContent}
      activeTools={activeTools}
      userAvatarUrl={userAvatarUrl}
    />,
  );
}

describe("ChatView — propagación del avatar de usuario", () => {
  it("propaga userAvatarUrl a la burbuja del mensaje de usuario", () => {
    const avatarUrl = "https://example.com/me.png";
    const { container } = renderChatView({
      messages: [userMessage],
      userAvatarUrl: avatarUrl,
    });

    expect(container.querySelector(`img[src="${avatarUrl}"]`)).not.toBeNull();
  });

  it("sin la prop userAvatarUrl no rompe el render y muestra UserOutlined", () => {
    const { container } = render(
      <ChatView
        messages={[userMessage]}
        loading={false}
        onSendMessage={vi.fn()}
        streaming={false}
        streamingContent=""
        activeTools={[]}
      />,
    );

    expect(container.querySelector(".anticon-user")).not.toBeNull();
  });

  it("la burbuja sintética de streaming muestra el avatar de Valet", () => {
    const { container } = renderChatView({
      messages: [],
      streaming: true,
      streamingContent: "Escribiendo...",
    });

    const logo = container.querySelector("img[src*='valet-icon']");
    expect(logo).not.toBeNull();
    expect(logo!.getAttribute("alt")).toBeTruthy();
  });
});

describe("ChatView — aprobación de herramientas destructivas", () => {
  const pendingApproval = {
    requestId: "req-1",
    toolName: "calendar",
    reason: "Eliminar un evento es irreversible",
  };

  function renderWithApproval(
    onResolveApproval: (approved: boolean) => void,
  ) {
    return render(
      <ChatView
        messages={[userMessage]}
        loading={false}
        onSendMessage={vi.fn()}
        streaming={false}
        streamingContent=""
        activeTools={[]}
        pendingApproval={pendingApproval}
        onResolveApproval={onResolveApproval}
      />,
    );
  }

  it("muestra el diálogo con la herramienta y el motivo", () => {
    renderWithApproval(vi.fn());

    expect(screen.getByText("calendar")).toBeInTheDocument();
    expect(
      screen.getByText("Eliminar un evento es irreversible"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Permitir" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Denegar" }),
    ).toBeInTheDocument();
  });

  it("pulsar Permitir llama a onResolveApproval(true)", async () => {
    const user = userEvent.setup();
    const onResolveApproval = vi.fn();
    renderWithApproval(onResolveApproval);

    await user.click(screen.getByRole("button", { name: "Permitir" }));

    expect(onResolveApproval).toHaveBeenCalledWith(true);
  });

  it("pulsar Denegar llama a onResolveApproval(false)", async () => {
    const user = userEvent.setup();
    const onResolveApproval = vi.fn();
    renderWithApproval(onResolveApproval);

    await user.click(screen.getByRole("button", { name: "Denegar" }));

    expect(onResolveApproval).toHaveBeenCalledWith(false);
  });

  it("sin pendingApproval no aparece el diálogo", () => {
    render(
      <ChatView
        messages={[userMessage]}
        loading={false}
        onSendMessage={vi.fn()}
        streaming={false}
        streamingContent=""
        activeTools={[]}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Permitir" }),
    ).not.toBeInTheDocument();
  });
});

describe("ChatView — respuesta interrumpida por un fallo del stream", () => {
  const partialText = "Hola, esto es una resp";

  function renderInterrupted() {
    const interrupted: Message = {
      id: "a-interrupted",
      role: "assistant",
      content: partialText,
      interrupted: true,
      created_at: "2026-10-24T10:31:00Z",
    };
    return render(
      <ChatView
        messages={[userMessage, interrupted]}
        loading={false}
        onSendMessage={vi.fn()}
        streaming={false}
        streamingContent=""
        activeTools={[]}
      />,
    );
  }

  it("mantiene el texto parcial visible y muestra el aviso de interrupción", () => {
    renderInterrupted();

    expect(screen.getByText(partialText)).toBeInTheDocument();
    expect(screen.getByText(/interrumpi/i)).toBeInTheDocument();
  });

  it("un mensaje de asistente normal no muestra el aviso de interrupción", () => {
    const normal: Message = {
      id: "a-normal",
      role: "assistant",
      content: "Respuesta completa",
      created_at: "2026-10-24T10:31:00Z",
    };
    render(
      <ChatView
        messages={[userMessage, normal]}
        loading={false}
        onSendMessage={vi.fn()}
        streaming={false}
        streamingContent=""
        activeTools={[]}
      />,
    );

    expect(screen.queryByText(/interrumpi/i)).not.toBeInTheDocument();
  });

  it("un error sin parcial muestra el aviso y no deja burbuja de asistente vacía", () => {
    const { container } = render(
      <ChatView
        messages={[userMessage]}
        loading={false}
        onSendMessage={vi.fn()}
        streaming={false}
        streamingContent=""
        activeTools={[]}
        error="Stream read error: boom"
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Stream read error: boom");
    expect(
      container.querySelectorAll("img[src*='valet-icon']"),
    ).toHaveLength(0);
  });
});
