/**
 * Registra el service worker de WebPush en `/sw.js` de forma tolerante.
 *
 * - Sin `navigator.serviceWorker` no hace nada.
 * - Si `register()` falla, el fallo se registra en consola y **no** se propaga:
 *   la aplicación sigue montada.
 */
export function registerServiceWorker(): void {
  if (!("serviceWorker" in navigator)) {
    return;
  }

  void navigator.serviceWorker.register("/sw.js").catch((error: unknown) => {
    console.error("No se pudo registrar el service worker:", error);
  });
}
