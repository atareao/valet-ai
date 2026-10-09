import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMainChat } from "../hooks/useMainChat";
import { api } from "../api/client";

// ---------------------------------------------------------------------------
// Hoisted shared storage — allows mock modules to capture SSE callbacks that
// the test can then invoke to simulate stream completion / error.
// ---------------------------------------------------------------------------
const { sseCallbacks } = vi.hoisted(() => ({
  sseCallbacks: {} as Record<string, unknown>,
}));
const { capturedBrowserContext } = vi.hoisted(() => ({
  capturedBrowserContext: { current: undefined as unknown },
}));

vi.mock("../api/client", () => ({
  api: {
    chatInit: vi.fn().mockResolvedValue({ messages: [], settings: {} }),
    listMessages: vi.fn(),
    getSettings: vi.fn(),
    approveAction: vi.fn().mockResolvedValue(undefined),
  },
  BASE_URL: "http://localhost:3000",
}));

vi.mock("../hooks/useSSE", () => ({
  useSSE: vi.fn(() => ({
    connect: vi.fn(
      (
        _content: string,
        options: {
          onChunk?: (c: string) => void;
          onDone?: (...args: string[]) => void;
          onError?: (m: string) => void;
          onToolCall?: (name: string, args: unknown) => void;
          onToolResult?: (name: string, success: boolean) => void;
          onApprovalRequired?: (
            requestId: string,
            toolName: string,
            reason: string,
          ) => void;
          onApprovalResult?: (requestId: string, approved: boolean) => void;
        },
        browserContext?: unknown,
      ) => {
        sseCallbacks.onChunk = options.onChunk;
        sseCallbacks.onDone = options.onDone;
        sseCallbacks.onError = options.onError;
        sseCallbacks.onToolCall = options.onToolCall;
        sseCallbacks.onToolResult = options.onToolResult;
        sseCallbacks.onApprovalRequired = options.onApprovalRequired;
        sseCallbacks.onApprovalResult = options.onApprovalResult;
        capturedBrowserContext.current = browserContext;
      },
    ),
    disconnect: vi.fn(),
    connected: false,
  })),
}));

describe("useMainChat", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Clear captured callbacks between tests
    sseCallbacks.onChunk = undefined;
    sseCallbacks.onDone = undefined;
    sseCallbacks.onError = undefined;
    sseCallbacks.onToolCall = undefined;
    sseCallbacks.onToolResult = undefined;
    sseCallbacks.onApprovalRequired = undefined;
    sseCallbacks.onApprovalResult = undefined;
  });

  // -----------------------------------------------------------------------
  // Loading initial messages from chatInit
  // -----------------------------------------------------------------------
  it("loads initial messages and settings on mount", async () => {
    const { result } = renderHook(() => useMainChat());

    // Initially loading is true
    expect(result.current.loading).toBe(true);

    // Wait for the initial load to finish
    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    // chatInit should have been called
    expect(api.chatInit).toHaveBeenCalledTimes(1);

    // Messages should be empty (as mocked)
    expect(result.current.messages).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  // -----------------------------------------------------------------------
  // Handles chatInit error gracefully
  // -----------------------------------------------------------------------
  it("handles chatInit error gracefully", async () => {
    vi.mocked(api.chatInit).mockRejectedValueOnce(new Error("Network error"));

    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    expect(result.current.error).toBe("Network error");
    expect(result.current.messages).toEqual([]);
  });

  // -----------------------------------------------------------------------
  // preserves user message when stream completes
  // -----------------------------------------------------------------------
  it("preserves user message when stream completes", async () => {
    const { result } = renderHook(() => useMainChat());

    // Wait for the initial load to finish
    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    // Send a message — this adds the optimistic user message and calls SSE.connect
    await act(async () => {
      result.current.sendMessage("Hola");
    });

    // After sendMessage, the optimistic user message should be in the list
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].role).toBe("user");
    expect(result.current.messages[0].content).toBe("Hola");
    expect(result.current.streaming).toBe(true);

    // Capture the optimistic ID so we can verify it survives the onDone call
    const optimisticId = result.current.messages[0].id;
    expect(optimisticId).toMatch(/^temp-/);

    // Simulate the SSE onDone callback
    await act(async () => {
      const doneFn = sseCallbacks.onDone as () => void;
      doneFn();
    });

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].content).toBe("Hola");
  });

  // -----------------------------------------------------------------------
  // preserves user message when stream errors
  // -----------------------------------------------------------------------
  it("preserves user message when stream errors", async () => {
    const { result } = renderHook(() => useMainChat());

    // Wait for initial load
    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    // Send a message
    await act(async () => {
      result.current.sendMessage("Hola");
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].role).toBe("user");

    // Simulate SSE onError
    await act(async () => {
      const errorFn = sseCallbacks.onError as (msg: string) => void;
      errorFn("Stream error");
    });

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].content).toBe("Hola");
    expect(result.current.error).toBe("Stream error");
  });

  // -----------------------------------------------------------------------
  // browser context passed to SSE connect
  // -----------------------------------------------------------------------
  it("sends message with browser context", async () => {
    const { result } = renderHook(() => useMainChat());

    // Wait for the initial load to finish
    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    // Send a message
    await act(async () => {
      result.current.sendMessage("Hola");
    });

    // The browser context should be passed as the 3rd argument to sse.connect
    expect(capturedBrowserContext.current).toBeTruthy();
    expect(capturedBrowserContext.current).toHaveProperty("timestamp");
    expect(capturedBrowserContext.current).toHaveProperty("timezone");
    expect(capturedBrowserContext.current).toHaveProperty("latitude");
    expect(capturedBrowserContext.current).toHaveProperty("longitude");
    expect(capturedBrowserContext.current).toHaveProperty("location_name");
  });

  // -----------------------------------------------------------------------
  // tracks active tools during streaming
  // -----------------------------------------------------------------------
  it("tracks active tools during streaming", async () => {
    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    expect(result.current.activeTools).toEqual([]);

    // Simulate tool call via the captured callback
    await act(async () => {
      const toolCallFn = sseCallbacks.onToolCall as (
        name: string,
        args: unknown,
      ) => void;
      if (toolCallFn) toolCallFn("search", { query: "test" });
    });

    expect(result.current.activeTools).toContain("search");
  });

  // -----------------------------------------------------------------------
  // dispatches events-changed on calendar tool result (not tool call)
  // -----------------------------------------------------------------------
  it("dispatches events-changed custom event on calendar tool result", async () => {
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    // Simulate calendar tool result (success)
    await act(async () => {
      const toolResultFn = sseCallbacks.onToolResult as (
        name: string,
        success: boolean,
      ) => void;
      if (toolResultFn) toolResultFn("calendar", true);
    });

    expect(dispatchSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "events-changed",
      }),
    );

    dispatchSpy.mockRestore();
  });

  it("does NOT dispatch events-changed on failed calendar tool result", async () => {
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    // Simulate calendar tool result with failure
    await act(async () => {
      const toolResultFn = sseCallbacks.onToolResult as (
        name: string,
        success: boolean,
      ) => void;
      if (toolResultFn) toolResultFn("calendar", false);
    });

    const eventsChangedCalls = dispatchSpy.mock.calls.filter(
      (args) => (args[0] as CustomEvent).type === "events-changed",
    );
    expect(eventsChangedCalls).toHaveLength(0);

    dispatchSpy.mockRestore();
  });

  it("does NOT dispatch events-changed on non-calendar tool results", async () => {
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    // Simulate a non-calendar tool result
    await act(async () => {
      const toolResultFn = sseCallbacks.onToolResult as (
        name: string,
        success: boolean,
      ) => void;
      if (toolResultFn) toolResultFn("search", true);
    });

    // dispatchEvent may have been called for other reasons, but never with 'events-changed'
    const eventsChangedCalls = dispatchSpy.mock.calls.filter(
      (args) => (args[0] as CustomEvent).type === "events-changed",
    );
    expect(eventsChangedCalls).toHaveLength(0);

    dispatchSpy.mockRestore();
  });

  // -----------------------------------------------------------------------
  // approval_required sets pendingApproval
  // -----------------------------------------------------------------------
  it("sets pendingApproval when approval_required arrives", async () => {
    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      const approvalFn = sseCallbacks.onApprovalRequired as (
        requestId: string,
        toolName: string,
        reason: string,
      ) => void;
      approvalFn("req-1", "calendar", "Borrar un evento es irreversible");
    });

    expect(result.current.pendingApproval).toEqual({
      requestId: "req-1",
      toolName: "calendar",
      reason: "Borrar un evento es irreversible",
    });
  });

  // -----------------------------------------------------------------------
  // resolveApproval posts to the backend and clears the pending state
  // -----------------------------------------------------------------------
  it("resolveApproval calls api.approveAction and clears pendingApproval", async () => {
    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      const approvalFn = sseCallbacks.onApprovalRequired as (
        requestId: string,
        toolName: string,
        reason: string,
      ) => void;
      approvalFn("req-2", "tasks", "Borrar una tarea es irreversible");
    });

    expect(result.current.pendingApproval).not.toBeNull();

    await act(async () => {
      await result.current.resolveApproval(true);
    });

    expect(api.approveAction).toHaveBeenCalledWith("req-2", true);
    expect(result.current.pendingApproval).toBeNull();
  });

  // -----------------------------------------------------------------------
  // resolveApproval keeps pendingApproval when the POST fails
  // -----------------------------------------------------------------------
  it("keeps pendingApproval when api.approveAction rejects", async () => {
    vi.mocked(api.approveAction).mockRejectedValueOnce(new Error("Boom"));

    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      const approvalFn = sseCallbacks.onApprovalRequired as (
        requestId: string,
        toolName: string,
        reason: string,
      ) => void;
      approvalFn("req-2b", "calendar", "motivo");
    });

    expect(result.current.pendingApproval).not.toBeNull();

    await act(async () => {
      await result.current.resolveApproval(true);
    });

    expect(api.approveAction).toHaveBeenCalledWith("req-2b", true);
    expect(result.current.pendingApproval).toEqual({
      requestId: "req-2b",
      toolName: "calendar",
      reason: "motivo",
    });
  });

  // -----------------------------------------------------------------------
  // pendingApproval is cleared by approval_result / done / error
  // -----------------------------------------------------------------------
  it("clears pendingApproval when approval_result arrives", async () => {
    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      const approvalFn = sseCallbacks.onApprovalRequired as (
        requestId: string,
        toolName: string,
        reason: string,
      ) => void;
      approvalFn("req-3", "calendar", "motivo");
    });

    expect(result.current.pendingApproval).not.toBeNull();

    await act(async () => {
      const resultFn = sseCallbacks.onApprovalResult as (
        requestId: string,
        approved: boolean,
      ) => void;
      resultFn("req-3", true);
    });

    expect(result.current.pendingApproval).toBeNull();
  });

  it("clears pendingApproval on done", async () => {
    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      const approvalFn = sseCallbacks.onApprovalRequired as (
        requestId: string,
        toolName: string,
        reason: string,
      ) => void;
      approvalFn("req-4", "calendar", "motivo");
    });

    expect(result.current.pendingApproval).not.toBeNull();

    await act(async () => {
      const doneFn = sseCallbacks.onDone as () => void;
      doneFn();
    });

    expect(result.current.pendingApproval).toBeNull();
  });

  it("clears pendingApproval on error", async () => {
    const { result } = renderHook(() => useMainChat());

    await vi.waitFor(
      () => {
        expect(result.current.loading).toBe(false);
      },
      { timeout: 3000 },
    );

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      const approvalFn = sseCallbacks.onApprovalRequired as (
        requestId: string,
        toolName: string,
        reason: string,
      ) => void;
      approvalFn("req-5", "calendar", "motivo");
    });

    expect(result.current.pendingApproval).not.toBeNull();

    await act(async () => {
      const errorFn = sseCallbacks.onError as (msg: string) => void;
      errorFn("Stream error");
    });

    expect(result.current.pendingApproval).toBeNull();
  });
});
