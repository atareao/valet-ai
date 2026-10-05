import React from "react";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { AuthProvider } from "../contexts/AuthProvider";
import { useAuth } from "../contexts/AuthContext";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Contrato (design, sección Frontend): `AuthProvider`/`useAuth` resuelve
 * `GET /api/auth/me` al montar, expone `loading` mientras está en curso, el
 * usuario con sus claims tras un 200 y `user: null` ante un 401.
 *
 * El `AuthProvider` actual es un placeholder con `loading: true` fijo, así
 * que los asserts de resolución fallan.
 */
function Probe() {
  const { user, loading } = useAuth();
  return (
    <div>
      <span data-testid="loading">{String(loading)}</span>
      <span data-testid="email">{user?.email ?? ""}</span>
    </div>
  );
}

describe("AuthContext / useAuth", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("expone loading en curso y resuelve el usuario tras un 200", async () => {
    let resolveFetch: (value: unknown) => void = () => undefined;
    const pending = new Promise<unknown>((resolve) => {
      resolveFetch = resolve;
    });
    const fetchMock = vi.fn().mockReturnValue(pending);
    vi.stubGlobal("fetch", fetchMock);

    render(
      <AuthProvider>
        <Probe />
      </AuthProvider>,
    );

    // Mientras `me` está en curso, seguimos en loading.
    expect(screen.getByTestId("loading")).toHaveTextContent("true");
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/api/auth/me"),
      expect.objectContaining({ credentials: "include" }),
    );

    await act(async () => {
      resolveFetch({
        ok: true,
        status: 200,
        json: async () => ({ sub: "u1", email: "a@b.c", name: "Lorenzo" }),
      });
    });

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("false"),
    );
    expect(screen.getByTestId("email")).toHaveTextContent("a@b.c");
  });

  it("deja al usuario no autenticado ante un 401", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        statusText: "Unauthorized",
        json: async () => ({ error: "unauthorized" }),
      }),
    );

    render(
      <AuthProvider>
        <Probe />
      </AuthProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("false"),
    );
    expect(screen.getByTestId("email")).toHaveTextContent("");
  });

  it("bajo StrictMode lanza un único GET /api/auth/me (caché por ref)", async () => {
    // La promesa se cachea en un ref (tarea 4.12) para que el doble montaje de
    // StrictMode no dispare dos `me`: ambos efectos comparten la misma promesa.
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ sub: "u1", email: "a@b.c", name: "Lorenzo" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(
      <React.StrictMode>
        <AuthProvider>
          <Probe />
        </AuthProvider>
      </React.StrictMode>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("false"),
    );

    const meCalls = fetchMock.mock.calls.filter(([url]) =>
      String(url).includes("/api/auth/me"),
    );
    expect(meCalls).toHaveLength(1);
  });

  it("useAuth fuera del provider lanza un error explícito", () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    expect(() => render(<Probe />)).toThrow();

    consoleError.mockRestore();
  });
});
