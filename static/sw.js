// Service worker de Valet.
//
// A propósito NO registra un handler de `fetch`: sin interceptar peticiones no
// puede servir ficheros en caché (quedaría obsoleto tras un despliegue) ni
// romper el HMR de Vite en desarrollo. Solo atiende `push` y
// `notificationclick`. Ver `openspec/changes/webpush-notifications/specs/push-ui`.

const ICON = "/icon-192.png";

self.addEventListener('push', (event) => {
  // Un push sin payload no muestra nada (es puramente informativo).
  if (!event.data) return;

  const payload = event.data.json();
  const title = payload.title || "Valet";

  event.waitUntil(
    self.registration.showNotification(title, {
      body: payload.body,
      icon: ICON,
      badge: ICON,
      data: { url: payload.url },
      tag: payload.tag,
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();

  const rawUrl = (event.notification.data && event.notification.data.url) || "/";
  const target = new URL(rawUrl, self.location.origin).href;

  event.waitUntil(
    self.clients
      .matchAll({ type: "window", includeUncontrolled: true })
      .then((clients) => {
        // Enfoca una pestaña ya abierta en esa URL si existe.
        for (const client of clients) {
          if (client.url === target && "focus" in client) {
            return client.focus();
          }
        }
        // Si no, abre una nueva.
        if (self.clients.openWindow) {
          return self.clients.openWindow(target);
        }
        return undefined;
      }),
  );
});
