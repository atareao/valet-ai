import { getVapidKey, subscribePush, unsubscribePush } from "../api/push";

/** Estado de las notificaciones WebPush en este navegador. */
export type PushStatus =
  | "unsupported"
  | "not-requested"
  | "denied"
  | "granted-unsubscribed"
  | "subscribed";

/**
 * Margen de espera para `navigator.serviceWorker.ready`. Esa promesa no rechaza
 * nunca y solo resuelve si hay un worker activo; sin este tope, un registro
 * fallido dejaría la UI en un `loading` permanente.
 */
const SERVICE_WORKER_READY_TIMEOUT_MS = 10_000;

/**
 * Espera a que el service worker esté activo, **con tope de tiempo**. Necesario
 * antes de suscribir, no para leer el estado.
 */
async function waitForServiceWorker(): Promise<ServiceWorkerRegistration> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () =>
        reject(
          new Error(
            "El service worker no está listo; recarga la página e inténtalo de nuevo.",
          ),
        ),
      SERVICE_WORKER_READY_TIMEOUT_MS,
    );
  });
  try {
    return await Promise.race([navigator.serviceWorker.ready, timeout]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

/**
 * Convierte una clave pública VAPID en base64url al `Uint8Array` que consume
 * `PushManager.subscribe({ applicationServerKey })`. Acepta el alfabeto
 * url-safe (`-`/`_`) y maneja la ausencia o presencia de padding.
 */
export function urlBase64ToUint8Array(
  base64String: string,
): Uint8Array<ArrayBuffer> {
  if (base64String.length === 0) {
    return new Uint8Array(new ArrayBuffer(0));
  }

  const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding)
    .replace(/-/g, "+")
    .replace(/_/g, "/");
  const raw = atob(base64);

  // Construimos sobre un `ArrayBuffer` explícito para que el tipo sea
  // `Uint8Array<ArrayBuffer>`, que es el que espera `applicationServerKey`
  // (`BufferSource`) en `lib.dom`. Un `new Uint8Array(n)` sin buffer da
  // `Uint8Array<ArrayBufferLike>`, no asignable.
  const output = new Uint8Array(new ArrayBuffer(raw.length));
  for (let i = 0; i < raw.length; i += 1) {
    output[i] = raw.charCodeAt(i);
  }
  return output;
}

/** ¿Este navegador puede recibir notificaciones WebPush? */
export function isPushSupported(): boolean {
  return (
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}

/**
 * Estado real: sin soporte, permiso aún no solicitado, permiso denegado,
 * permiso concedido sin suscripción o suscrito. Consulta la suscripción viva del
 * `PushManager`, no un valor cacheado. NO espera a `serviceWorker.ready` —esa
 * promesa puede no resolverse nunca—: usa `getRegistration()`, que resuelve
 * aunque no haya registro.
 */
export async function getPushStatus(): Promise<PushStatus> {
  if (!isPushSupported()) return "unsupported";
  if (Notification.permission === "denied") return "denied";
  if (Notification.permission === "default") return "not-requested";

  const registration = await navigator.serviceWorker.getRegistration();
  const subscription = await registration?.pushManager.getSubscription();
  return subscription ? "subscribed" : "granted-unsubscribed";
}

/**
 * Activa las notificaciones. Pide el permiso **aquí y solo aquí**, tras el
 * gesto del usuario: si no queda concedido lanza un error legible y no publica
 * nada. Con el permiso concedido obtiene la clave pública VAPID, suscribe el
 * navegador y publica la suscripción en el backend.
 */
export async function enablePushNotifications(): Promise<void> {
  if (!isPushSupported()) {
    throw new Error("Este navegador no soporta notificaciones push.");
  }

  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new Error(
      "No se concedió el permiso de notificaciones; actívalas en el navegador.",
    );
  }

  const { public_key } = await getVapidKey();
  const registration = await waitForServiceWorker();
  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: urlBase64ToUint8Array(public_key),
  });

  const { endpoint, keys } = subscription.toJSON();
  if (!endpoint || !keys?.p256dh || !keys.auth) {
    throw new Error("El navegador no devolvió las claves de la suscripción.");
  }

  try {
    await subscribePush({
      endpoint,
      keys: { p256dh: keys.p256dh, auth: keys.auth },
    });
  } catch (error) {
    // El navegador ya tiene suscripción viva pero el backend no la conoce:
    // dejaría la UI mintiendo («activadas» sin avisos). La deshacemos.
    await subscription.unsubscribe().catch(() => undefined);
    throw error;
  }
}

/**
 * Desactiva las notificaciones: borra la suscripción en el backend y después
 * cancela la del navegador. El orden importa: si algo falla a medias, el estado
 * consistente es «el backend no tiene fila» (no llegan avisos), inofensivo,
 * en lugar de una fila obsoleta apuntando a un navegador que ya no la tiene.
 * Sin suscripción no hace nada.
 */
export async function disablePushNotifications(): Promise<void> {
  if (!isPushSupported()) return;

  const registration = await navigator.serviceWorker.getRegistration();
  const subscription = await registration?.pushManager.getSubscription();
  if (!subscription) return;

  const endpoint = subscription.endpoint;
  await unsubscribePush(endpoint);
  await subscription.unsubscribe();
}
