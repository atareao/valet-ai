import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getVapidKey, subscribePush, unsubscribePush } from "../api/push";
import {
  disablePushNotifications,
  enablePushNotifications,
  getPushStatus,
  urlBase64ToUint8Array,
} from "./pushManager";
import type { PushSubscriptionPayload } from "../types";

vi.mock("../api/push", () => ({
  getVapidKey: vi.fn(),
  subscribePush: vi.fn(),
  unsubscribePush: vi.fn(),
}));

const mockedGetVapidKey = vi.mocked(getVapidKey);
const mockedSubscribePush = vi.mocked(subscribePush);
const mockedUnsubscribePush = vi.mocked(unsubscribePush);

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

/** Registra un service worker falso en `navigator`. */
function installServiceWorker(registration: ServiceWorkerRegistration): void {
  Object.defineProperty(navigator, "serviceWorker", {
    configurable: true,
    value: {
      ready: Promise.resolve(registration),
      getRegistration: () => Promise.resolve(registration),
    },
  });
}

/** Retira el soporte de push del navegador (caso `unsupported`). */
function uninstallPushSupport(): void {
  Reflect.deleteProperty(navigator, "serviceWorker");
  Reflect.deleteProperty(window, "PushManager");
}

/** Registra soporte de `PushManager` y `Notification` en `window`. */
function installNotification(
  requestPermission: () => Promise<NotificationPermission>,
  permission: NotificationPermission = "default",
): void {
  Object.defineProperty(window, "PushManager", {
    configurable: true,
    writable: true,
    value: class FakePushManager {},
  });
  Object.defineProperty(window, "Notification", {
    configurable: true,
    writable: true,
    value: { permission, requestPermission: vi.fn(requestPermission) },
  });
}

describe("urlBase64ToUint8Array", () => {
  it("convierte una cadena url-safe sin padding a los bytes correctos", () => {
    // "-_8" en base64url = [0xFB, 0xFF] en binario.
    expect(Array.from(urlBase64ToUint8Array("-_8"))).toEqual([251, 255]);
  });

  it("no añade padding cuando la longitud ya es múltiplo de 4", () => {
    expect(Array.from(urlBase64ToUint8Array("AQID"))).toEqual([1, 2, 3]);
  });

  it("convierte la cadena vacía en un array vacío", () => {
    expect(Array.from(urlBase64ToUint8Array(""))).toEqual([]);
  });
});

describe("enablePushNotifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetVapidKey.mockResolvedValue({ public_key: "AQID" });
    mockedSubscribePush.mockResolvedValue({ id: "sub-1" });
    mockedUnsubscribePush.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("con permiso concedido suscribe el navegador y publica en el backend", async () => {
    const subscribe = vi.fn().mockResolvedValue(fakeSubscription("https://push.test/abc"));
    installServiceWorker({
      pushManager: { getSubscription: vi.fn(), subscribe },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("granted"));

    await enablePushNotifications();

    expect(window.Notification.requestPermission).toHaveBeenCalledTimes(1);
    expect(mockedGetVapidKey).toHaveBeenCalledTimes(1);
    expect(subscribe).toHaveBeenCalledTimes(1);
    expect(mockedSubscribePush).toHaveBeenCalledWith({
      endpoint: "https://push.test/abc",
      keys: { p256dh: "p256dh-key", auth: "auth-secret" },
    });
  });

  it("con permiso denegado lanza un error legible y no publica nada", async () => {
    const subscribe = vi.fn();
    installServiceWorker({
      pushManager: { getSubscription: vi.fn(), subscribe },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("denied"));

    await expect(enablePushNotifications()).rejects.toThrow(/permiso/i);

    expect(subscribe).not.toHaveBeenCalled();
    expect(mockedSubscribePush).not.toHaveBeenCalled();
  });

  it("si el POST al backend falla, deshace la suscripción del navegador", async () => {
    const subscription = fakeSubscription("https://push.test/abc");
    const subscribe = vi.fn().mockResolvedValue(subscription);
    installServiceWorker({
      pushManager: { getSubscription: vi.fn(), subscribe },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("granted"));
    mockedSubscribePush.mockRejectedValueOnce(new Error("backend caído"));

    await expect(enablePushNotifications()).rejects.toThrow("backend caído");

    expect(subscription.unsubscribe).toHaveBeenCalledTimes(1);
  });
});

describe("disablePushNotifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedUnsubscribePush.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("cancela la suscripción del navegador y borra en el backend", async () => {
    const subscription = fakeSubscription("https://push.test/abc");
    installServiceWorker({
      pushManager: { getSubscription: vi.fn().mockResolvedValue(subscription), subscribe: vi.fn() },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("granted"));

    await disablePushNotifications();

    expect(subscription.unsubscribe).toHaveBeenCalledTimes(1);
    expect(mockedUnsubscribePush).toHaveBeenCalledWith("https://push.test/abc");
  });

  it("si el DELETE al backend falla, no desuscribe el navegador", async () => {
    const subscription = fakeSubscription("https://push.test/abc");
    installServiceWorker({
      pushManager: { getSubscription: vi.fn().mockResolvedValue(subscription), subscribe: vi.fn() },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("granted"));
    mockedUnsubscribePush.mockRejectedValueOnce(new Error("backend caído"));

    await expect(disablePushNotifications()).rejects.toThrow("backend caído");

    expect(subscription.unsubscribe).not.toHaveBeenCalled();
  });
});

describe("getPushStatus", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("devuelve `unsupported` sin soporte del navegador", async () => {
    uninstallPushSupport();

    await expect(getPushStatus()).resolves.toBe("unsupported");
  });

  it("devuelve `not-requested` cuando el permiso aún no se ha pedido", async () => {
    installServiceWorker({
      pushManager: {
        getSubscription: vi.fn().mockResolvedValue(null),
        subscribe: vi.fn(),
      },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("default"), "default");

    await expect(getPushStatus()).resolves.toBe("not-requested");
  });

  it("devuelve `denied` cuando el permiso está denegado", async () => {
    installServiceWorker({
      pushManager: {
        getSubscription: vi.fn().mockResolvedValue(null),
        subscribe: vi.fn(),
      },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("denied"), "denied");

    await expect(getPushStatus()).resolves.toBe("denied");
  });

  it("devuelve `granted-unsubscribed` con permiso concedido y sin suscripción", async () => {
    installServiceWorker({
      pushManager: {
        getSubscription: vi.fn().mockResolvedValue(null),
        subscribe: vi.fn(),
      },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("granted"), "granted");

    await expect(getPushStatus()).resolves.toBe("granted-unsubscribed");
  });

  it("devuelve `subscribed` cuando ya existe una suscripción", async () => {
    installServiceWorker({
      pushManager: {
        getSubscription: vi.fn().mockResolvedValue(fakeSubscription("https://push.test/abc")),
        subscribe: vi.fn(),
      },
    } as unknown as ServiceWorkerRegistration);
    installNotification(() => Promise.resolve("granted"), "granted");

    await expect(getPushStatus()).resolves.toBe("subscribed");
  });
});
