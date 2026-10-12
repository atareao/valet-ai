import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import type { ReactNode } from "react";

// Mock **parcial** del cliente API (patrón de `StravaIntegration.test.tsx`):
// solo se sustituye `request`, que es la capa que usan `getVapidKey`,
// `subscribePush` y `unsubscribePush` de `api/push.ts`. Así el `pushManager`
// real corre de verdad y la aserción de «no se publica nada» es significativa.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return { ...actual, request: vi.fn() };
});

// Mock **parcial** del gestor de push: se conserva la implementación real salvo
// `isPushSupported`, que se controla desde cada prueba para el caso «sin
// soporte» sin depender de borrar `window.PushManager`.
vi.mock("../push/pushManager", async () => {
  const actual =
    await vi.importActual<typeof import("../push/pushManager")>(
      "../push/pushManager",
    );
  return { ...actual, isPushSupported: vi.fn(() => true) };
});

import { ApiError, request } from "../api/client";
import { isPushSupported } from "../push/pushManager";
import { NotificationsTab } from "./NotificationsTab";
import type { PushSubscriptionPayload } from "../types";

const mockRequest = vi.mocked(request);
const mockIsPushSupported = vi.mocked(isPushSupported);

const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

/** Construye un `PushSubscription` falso con el mínimo que usa el gestor. */
function fakeSubscription(endpoint: string): PushSubscription {
  const payload: PushSubscriptionPayload = {
    endpoint,
    keys: { p256dh: "p256dh-key", auth: "auth-secret" },
  };
  return {
    endpoint,
    toJSON: () => payload,
    unsubscribe: vi.fn().mockResolvedValue(true),
  } as unknown as PushSubscription;
}

interface BrowserOptions {
  permission?: NotificationPermission;
  requestPermission?: () => Promise<NotificationPermission>;
  getSubscription?: () => Promise<PushSubscription | null>;
  subscribe?: () => Promise<PushSubscription>;
}

/** Instala un navegador falso con `serviceWorker`, `PushManager` y `Notification`. */
function installBrowser(options: BrowserOptions = {}): void {
  const getSubscription =
    options.getSubscription ?? (() => Promise.resolve(null));
  const subscribe =
    options.subscribe ?? (() => Promise.resolve(fakeSubscription("https://push.test/new")));
  const registration = {
    pushManager: {
      getSubscription: vi.fn(getSubscription),
      subscribe: vi.fn(subscribe),
    },
  } as unknown as ServiceWorkerRegistration;
  Object.defineProperty(navigator, "serviceWorker", {
    configurable: true,
    value: {
      ready: Promise.resolve(registration),
      getRegistration: () => Promise.resolve(registration),
    },
  });
  Object.defineProperty(window, "PushManager", {
    configurable: true,
    writable: true,
    value: class FakePushManager {},
  });
  const notification = {
    permission: options.permission ?? "default",
    requestPermission: vi.fn(() => {
      const result = options.requestPermission
        ? options.requestPermission()
        : Promise.resolve("granted" as const);
      return result.then((decision) => {
        notification.permission = decision;
        return decision;
      });
    }),
  };
  Object.defineProperty(window, "Notification", {
    configurable: true,
    writable: true,
    value: notification,
  });
}

const renderTab = (): void => {
  render(<NotificationsTab />, { wrapper: AppWrapper });
};

describe("NotificationsTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsPushSupported.mockReturnValue(true);
    // Respuesta combinada: sirve tanto para `getVapidKey` (lee `public_key`)
    // como para `subscribePush` (lee `id`); `unsubscribePush` la ignora.
    mockRequest.mockResolvedValue({ public_key: "AQID", id: "sub-1" } as never);
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
    vi.restoreAllMocks();
  });

  it("refleja «suscrito» al cargar cuando ya existe una suscripción", async () => {
    installBrowser({
      permission: "granted",
      getSubscription: () =>
        Promise.resolve(fakeSubscription("https://push.test/abc")),
    });

    renderTab();

    await waitFor(() => {
      expect(screen.getByRole("switch")).toBeChecked();
    });
  });

  it("activar pide permiso, suscribe y publica la suscripción", async () => {
    const user = userEvent.setup();
    const subscription = fakeSubscription("https://push.test/new");
    let subscribed = false;
    installBrowser({
      permission: "default",
      requestPermission: () => Promise.resolve("granted"),
      getSubscription: () => Promise.resolve(subscribed ? subscription : null),
      subscribe: () => {
        subscribed = true;
        return Promise.resolve(subscription);
      },
    });

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await waitFor(() => expect(switchEl).not.toBeChecked());
    await user.click(switchEl);

    await waitFor(() => expect(switchEl).toBeChecked());
    expect(mockRequest).toHaveBeenCalledWith(
      "/push/subscriptions",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("con el permiso denegado muestra un aviso y no publica nada", async () => {
    const user = userEvent.setup();
    installBrowser({
      permission: "denied",
      requestPermission: () => Promise.resolve("denied"),
      getSubscription: () => Promise.resolve(null),
    });

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await user.click(switchEl);

    expect(
      await screen.findByText(/No se concedió el permiso de notificaciones/i),
    ).toBeInTheDocument();
    expect(switchEl).not.toBeChecked();
    expect(mockRequest).not.toHaveBeenCalled();
  });

  it("desactivar cancela la suscripción y la borra en el servidor", async () => {
    const user = userEvent.setup();
    const subscription = fakeSubscription("https://push.test/abc");
    let subscribed = true;
    installBrowser({
      permission: "granted",
      getSubscription: () => Promise.resolve(subscribed ? subscription : null),
    });
    subscription.unsubscribe = vi.fn().mockImplementation(() => {
      subscribed = false;
      return Promise.resolve(true);
    });

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await waitFor(() => expect(switchEl).toBeChecked());
    await user.click(switchEl);

    await waitFor(() => expect(switchEl).not.toBeChecked());
    expect(subscription.unsubscribe).toHaveBeenCalledTimes(1);
    expect(mockRequest).toHaveBeenCalledWith(
      "/push/subscriptions",
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("sin PushManager deshabilita el switch y muestra una explicación", async () => {
    mockIsPushSupported.mockReturnValue(false);

    renderTab();

    const switchEl = await screen.findByRole("switch");
    expect(switchEl).toBeDisabled();
    expect(
      screen.getByText(/no soporta notificaciones push/i),
    ).toBeInTheDocument();
  });

  it("con el permiso aún no solicitado lo dice y deja el switch apagado", async () => {
    installBrowser({ permission: "default", getSubscription: () => Promise.resolve(null) });

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await waitFor(() => expect(switchEl).not.toBeChecked());
    expect(
      await screen.findByText(/todavía no has dado permiso/i),
    ).toBeInTheDocument();
  });

  it("con el permiso denegado en la carga muestra el aviso correspondiente", async () => {
    installBrowser({ permission: "denied", getSubscription: () => Promise.resolve(null) });

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await waitFor(() => expect(switchEl).not.toBeChecked());
    expect(await screen.findByText(/denegado/i)).toBeInTheDocument();
  });

  it("con permiso concedido sin suscripción invita a activar el interruptor", async () => {
    installBrowser({ permission: "granted", getSubscription: () => Promise.resolve(null) });

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await waitFor(() => expect(switchEl).not.toBeChecked());
    expect(await screen.findByText(/permiso concedido/i)).toBeInTheDocument();
  });

  it("traduce el 503 `push_unavailable` a un mensaje humano", async () => {
    const user = userEvent.setup();
    installBrowser({
      permission: "default",
      requestPermission: () => Promise.resolve("granted"),
      getSubscription: () => Promise.resolve(null),
    });
    mockRequest
      .mockResolvedValueOnce({ public_key: "AQID" } as never)
      .mockRejectedValueOnce(new ApiError("push_unavailable", 503));

    renderTab();

    const switchEl = await screen.findByRole("switch");
    await user.click(switchEl);

    expect(
      await screen.findByText(/no están disponibles en el servidor/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/push_unavailable/)).not.toBeInTheDocument();
  });
});
