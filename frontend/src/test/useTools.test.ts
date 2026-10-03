import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el cliente API (no `fetch`) siguiendo el patrón de
// `usePersistentMemory.test.ts`. El módulo `../hooks/useTools` aún no existe:
// este import es la causa roja esperada de la fase RED.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      getTools: vi.fn(),
      toggleTool: vi.fn(),
    },
  };
});

import { api } from "../api/client";
import { useTools } from "../hooks/useTools";

const mockGetTools = vi.mocked(api.getTools);
const mockToggleTool = vi.mocked(api.toggleTool);

// Espejo local del contrato `Tool` ({ id, name, description, enabled }) que
// aún no existe en `src/types`. Evita acoplar el test a un tipo inexistente.
interface MockTool {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}

const toolsFixture: MockTool[] = [
  {
    id: "weather",
    name: "Weather",
    description: "Consulta el tiempo",
    enabled: true,
  },
  {
    id: "search",
    name: "Search",
    description: "Búsqueda web",
    enabled: true,
  },
];

describe("useTools", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetTools.mockResolvedValue(toolsFixture);
  });

  it("carga las tools al montar y expone tools/loading", async () => {
    const { result } = renderHook(() => useTools());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(mockGetTools).toHaveBeenCalledTimes(1);
    expect(result.current.tools).toEqual(toolsFixture);
    expect(result.current.error).toBeNull();
  });

  it("toggle(id) llama a api.toggleTool(id) y actualiza esa tool con la respuesta", async () => {
    mockToggleTool.mockResolvedValue({ ...toolsFixture[0], enabled: false });
    const { result } = renderHook(() => useTools());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.toggle("weather");
    });

    expect(mockToggleTool).toHaveBeenCalledWith("weather");
    expect(
      result.current.tools.find((tool: MockTool) => tool.id === "weather")
        ?.enabled,
    ).toBe(false);
    // El resto de tools no se altera.
    expect(
      result.current.tools.find((tool: MockTool) => tool.id === "search")
        ?.enabled,
    ).toBe(true);
  });

  it("si toggle rechaza fija error y conserva el estado previo", async () => {
    mockToggleTool.mockRejectedValue(new Error("boom"));
    const { result } = renderHook(() => useTools());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.toggle("weather");
    });

    expect(result.current.error).toBeTruthy();
    expect(result.current.tools).toEqual(toolsFixture);
  });

  it("un toggle correcto tras un fallo limpia el error", async () => {
    mockToggleTool.mockRejectedValueOnce(new Error("boom"));
    const { result } = renderHook(() => useTools());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.toggle("weather");
    });
    expect(result.current.error).toBeTruthy();

    mockToggleTool.mockResolvedValueOnce({ ...toolsFixture[0], enabled: false });
    await act(async () => {
      await result.current.toggle("weather");
    });

    expect(result.current.error).toBeNull();
  });

  it("al iniciar un toggle limpia el error previo antes de que resuelva", async () => {
    mockToggleTool.mockRejectedValueOnce(new Error("boom"));
    const { result } = renderHook(() => useTools());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.toggle("weather");
    });
    expect(result.current.error).toBeTruthy();

    // Segundo intento que queda pendiente: el error debe limpiarse al inicio
    // (antes del await), de modo que un fallo posterior con el mismo mensaje
    // vuelva a transicionar null → mensaje y el `useEffect` vuelva a avisar.
    let resolveSecond: (tool: MockTool) => void = () => {};
    mockToggleTool.mockImplementationOnce(
      () =>
        new Promise<MockTool>((resolve) => {
          resolveSecond = resolve;
        }),
    );

    await act(async () => {
      void result.current.toggle("weather");
    });
    expect(result.current.error).toBeNull();

    await act(async () => {
      resolveSecond({ ...toolsFixture[0], enabled: false });
    });
  });
});