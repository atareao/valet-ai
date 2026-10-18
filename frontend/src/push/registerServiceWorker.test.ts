import { afterEach, describe, expect, it, vi } from "vitest";

import { registerServiceWorker } from "./registerServiceWorker";

/** Instala un `navigator.serviceWorker` falso con el `register` dado. */
function installServiceWorker(
  register: (url: string) => Promise<ServiceWorkerRegistration>,
): void {
  Object.defineProperty(navigator, "serviceWorker", {
    configurable: true,
    value: { register },
  });
}

/** Quita `navigator.serviceWorker` para simular un navegador sin soporte. */
function removeServiceWorker(): void {
  Reflect.deleteProperty(navigator, "serviceWorker");
}

describe("registerServiceWorker", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    removeServiceWorker();
  });

  it("con soporte llama a register('/sw.js')", () => {
    const register = vi
      .fn<(url: string) => Promise<ServiceWorkerRegistration>>()
      .mockResolvedValue({} as ServiceWorkerRegistration);
    installServiceWorker(register);

    registerServiceWorker();

    expect(register).toHaveBeenCalledTimes(1);
    expect(register).toHaveBeenCalledWith("/sw.js");
  });

  it("sin navigator.serviceWorker no llama a register ni lanza error", () => {
    const register = vi
      .fn<(url: string) => Promise<ServiceWorkerRegistration>>()
      .mockResolvedValue({} as ServiceWorkerRegistration);
    installServiceWorker(register);
    removeServiceWorker();

    expect(() => registerServiceWorker()).not.toThrow();
    expect(register).not.toHaveBeenCalled();
  });

  it("un register que rechaza no propaga el error ni deja una promesa sin manejar", async () => {
    const register = vi
      .fn<(url: string) => Promise<ServiceWorkerRegistration>>()
      .mockRejectedValue(new Error("boom"));
    installServiceWorker(register);

    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    const onUnhandled = vi.fn();
    window.addEventListener("unhandledrejection", onUnhandled);

    expect(() => registerServiceWorker()).not.toThrow();

    // Deja que la promesa rechazada se asiente: si no se capturase, aquí
    // saltaría un `unhandledrejection`.
    await new Promise((resolve) => setTimeout(resolve, 0));

    window.removeEventListener("unhandledrejection", onUnhandled);
    expect(onUnhandled).not.toHaveBeenCalled();
    expect(consoleError).toHaveBeenCalled();
  });
});
