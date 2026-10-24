import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChatView } from "../components/ChatView";
import type { Message } from "../types";

/**
 * Fase RED de `chat-cross-device-sync` (spec `frontend`).
 *
 * Requisito: el cliente SHALL mostrar que hay un turno en curso en otro
 * dispositivo, y SHALL avisar de un `409` sin dar el mensaje por enviado.
 */

const userMessage: Message = {
  id: "u1",
  role: "user",
  content: "Hola",
  created_at: "2026-10-24T10:30:00Z",
};

beforeEach(() => {
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

interface Overrides {
  remoteBusy?: boolean;
  error?: string | null;
  streaming?: boolean;
  messages?: Message[];
  rejectedDraft?: string | null;
}

function chatView(overrides: Overrides = {}) {
  const {
    remoteBusy = false,
    error = null,
    streaming = false,
    messages = [userMessage],
    rejectedDraft = null,
  } = overrides;

  return (
    <ChatView
      messages={messages}
      loading={false}
      onSendMessage={vi.fn()}
      streaming={streaming}
      streamingContent=""
      activeTools={[]}
      remoteBusy={remoteBusy}
      error={error}
      rejectedDraft={rejectedDraft}
    />
  );
}

function renderChatView(overrides: Overrides = {}) {
  return render(chatView(overrides));
}

describe("ChatView — indicador de turno en otro dispositivo", () => {
  it("aparece mientras el otro dispositivo responde", () => {
    renderChatView({ remoteBusy: true });

    expect(screen.getByText(/otro dispositivo/i)).toBeInTheDocument();
  });

  it("no aparece cuando no hay turno ajeno en curso", () => {
    renderChatView({ remoteBusy: false });

    expect(screen.queryByText(/otro dispositivo/i)).not.toBeInTheDocument();
  });

  it("desaparece al terminar el turno ajeno", () => {
    const { rerender } = renderChatView({ remoteBusy: true });
    expect(screen.getByText(/otro dispositivo/i)).toBeInTheDocument();

    rerender(chatView({ remoteBusy: false }));

    expect(screen.queryByText(/otro dispositivo/i)).not.toBeInTheDocument();
  });

  it("la región viva existe siempre, para poder anunciar la aparición del texto", () => {
    const { rerender } = renderChatView({ remoteBusy: false });

    // El contenedor `role="status"` no puede nacer con el texto: un lector de
    // pantalla (NVDA/JAWS) no anuncia una región insertada junto con su
    // contenido. Debe estar en el DOM desde el principio y cambiar solo el texto.
    const region = screen.getByRole("status");
    expect(region).toHaveAttribute("aria-live", "polite");
    expect(region).not.toHaveTextContent(/otro dispositivo/i);

    rerender(chatView({ remoteBusy: true }));

    expect(screen.getByRole("status")).toHaveTextContent(/otro dispositivo/i);
  });

  it("el icono de carga queda oculto a los lectores de pantalla", () => {
    renderChatView({ remoteBusy: true });

    const icon = screen.getByRole("status").querySelector(".anticon");
    expect(icon).not.toBeNull();
    expect(icon).toHaveAttribute("aria-hidden", "true");
  });
});

describe("ChatView — aviso de chat ocupado", () => {
  it("muestra el mensaje de error del servidor", () => {
    renderChatView({ error: "El chat está ocupado por otro turno" });

    expect(
      screen.getByText("El chat está ocupado por otro turno"),
    ).toBeInTheDocument();
  });

  it("sin error no hay aviso", () => {
    renderChatView({ error: null });

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("ChatView — el texto de un envío rechazado no se pierde", () => {
  const draft = "Un párrafo que no quiero perder";

  it("devuelve al campo de entrada el texto rechazado", () => {
    const { rerender } = renderChatView({ rejectedDraft: null });

    const field = screen.getByPlaceholderText(/Escribe un mensaje a Valet/i);
    expect(field).toHaveValue("");

    rerender(chatView({ rejectedDraft: draft }));

    expect(
      screen.getByPlaceholderText(/Escribe un mensaje a Valet/i),
    ).toHaveValue(draft);
  });

  it("reponer no es limpiar: un `null` no pisa lo que el usuario escribe", async () => {
    const user = userEvent.setup();
    const { rerender } = renderChatView({ rejectedDraft: null });

    const field = screen.getByPlaceholderText(/Escribe un mensaje a Valet/i);
    await user.type(field, "otra cosa");

    // Llega otro aviso cualquiera, sin texto que reponer.
    rerender(chatView({ error: "otro aviso", rejectedDraft: null }));

    expect(field).toHaveValue("otra cosa");
  });
});
