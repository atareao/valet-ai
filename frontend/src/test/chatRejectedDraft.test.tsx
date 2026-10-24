import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useMainChat } from "../hooks/useMainChat";
import { ChatView } from "../components/ChatView";

/**
 * `chat-cross-device-sync` — «Un envío con el turno ocupado SHALL avisarse **sin
 * perder el texto**».
 *
 * Prueba de extremo a extremo del lado del cliente: se teclea en el campo de
 * entrada, se envía, el servidor rechaza el envío con `409` y el texto
 * tecleado **vuelve al campo** (y no queda en la lista como enviado), listo
 * para reenviarlo cuando el turno ajeno termine.
 *
 * `useSSE` y el cliente HTTP se mockean; la suscripción al bus de eventos
 * (`useChatSync`) es la implementación real, alimentada por un
 * `ReadableStream` falso (mismo patrón que `useMainChat.sync.test.ts`).
 */

interface SseCallbacks {
  onChunk?: (chunk: string) => void;
  onDone?: (...args: unknown[]) => void;
  onError?: (message: string, status?: number) => void;
}

const { sseCallbacks } = vi.hoisted(() => ({
  sseCallbacks: {} as SseCallbacks,
}));

vi.mock("../api/client", () => ({
  api: {
    chatInit: vi.fn().mockResolvedValue({ messages: [], settings: {} }),
    listMessages: vi.fn(),
    getSettings: vi.fn(),
    approveAction: vi.fn().mockResolvedValue(undefined),
  },
  BASE_URL: "http://localhost:3000",
  UNAUTHORIZED_EVENT: "valet:unauthorized",
  chatEventsUrl: (since?: string | null) =>
    `http://localhost:3000/chat/events${
      since ? `?since=${encodeURIComponent(since)}` : ""
    }`,
}));

vi.mock("../hooks/useSSE", () => ({
  useSSE: vi.fn(() => ({
    connect: vi.fn(
      (
        _content: string,
        options: SseCallbacks,
        _browserContext?: unknown,
        _clientMessageId?: string,
      ) => {
        sseCallbacks.onChunk = options.onChunk;
        sseCallbacks.onDone = options.onDone;
        sseCallbacks.onError = options.onError;
      },
    ),
    disconnect: vi.fn(),
    connected: false,
  })),
}));

const encoder = new TextEncoder();

/** Stream SSE falso: se le empujan eventos y se cierra cuando haga falta. */
function fakeStream() {
  const pending: string[] = [];
  let waiter: ((result: { done: boolean; value?: Uint8Array }) => void) | null =
    null;
  let ended = false;

  const next = (): Promise<{ done: boolean; value?: Uint8Array }> => {
    const chunk = pending.shift();
    if (chunk !== undefined) {
      return Promise.resolve({ done: false, value: encoder.encode(chunk) });
    }
    if (ended) return Promise.resolve({ done: true, value: undefined });
    return new Promise((resolve) => {
      waiter = resolve;
    });
  };

  const reader = {
    read: vi.fn(next),
    cancel: vi.fn(async () => {
      ended = true;
      waiter?.({ done: true, value: undefined });
      waiter = null;
    }),
  };

  return {
    response: {
      ok: true,
      status: 200,
      statusText: "OK",
      body: { getReader: () => reader },
      json: async () => ({}),
    } as unknown as Response,
  };
}

/** Chat real (`useMainChat`) pintado por `ChatView`, como en `AppLayout`. */
function Harness() {
  const chat = useMainChat();
  return (
    <ChatView
      messages={chat.messages}
      loading={chat.loading}
      onSendMessage={chat.sendMessage}
      streaming={chat.streaming}
      streamingContent={chat.streamingContent}
      activeTools={chat.activeTools}
      remoteBusy={chat.remoteBusy}
      error={chat.error}
      rejectedDraft={chat.rejectedDraft}
    />
  );
}

const draft = "Un párrafo que no quiero perder";
const busyError = "El chat está ocupado por otro turno";

beforeEach(() => {
  sseCallbacks.onChunk = undefined;
  sseCallbacks.onDone = undefined;
  sseCallbacks.onError = undefined;

  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(fakeStream().response));

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

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Un envío con el turno ocupado no pierde el texto", () => {
  it("tras un 409 el texto vuelve al campo de entrada y no queda como enviado", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await waitFor(() =>
      expect(screen.queryByText("Cargando...")).not.toBeInTheDocument(),
    );

    const field = screen.getByPlaceholderText(/Escribe un mensaje a Valet/i);

    /** Pulsa enviar: el campo se vacía y el `connect` del turno queda armado. */
    const startSend = async () => {
      await user.click(screen.getByRole("button"));
      await waitFor(() => expect(field).toHaveValue(""));
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    };

    /** El servidor rechaza el envío: hay un turno en curso en otro dispositivo. */
    const reject = async () => {
      await act(async () => {
        sseCallbacks.onError?.(busyError, 409);
      });
    };

    const fieldValue = () =>
      expect(
        screen.getByPlaceholderText(/Escribe un mensaje a Valet/i),
      ).toHaveValue(draft);

    await user.type(field, draft);
    await startSend();

    // La burbuja optimista se pinta con el texto tecleado (el campo ya está
    // vacío, así que el único nodo con ese texto es la burbuja).
    const optimisticBubble = screen.getByText(draft);

    await reject();

    // Aviso legible y mensaje que **no** queda como enviado…
    expect(screen.getByRole("alert")).toHaveTextContent(busyError);
    expect(optimisticBubble).not.toBeInTheDocument();

    // …y, sobre todo, el texto tecleado vuelve al campo para poder reenviarlo.
    await waitFor(fieldValue);

    // Un reenvío del mismo texto vuelve a rechazarse: sigue sin perderse (el
    // hook limpia el aviso al empezar el envío, así que la reposición ocurre
    // aunque el contenido sea idéntico al de la vez anterior).
    await startSend();
    await reject();

    expect(screen.getByRole("alert")).toHaveTextContent(busyError);
    await waitFor(fieldValue);
  });
});
