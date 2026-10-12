import { describe, expect, it } from "vitest";

// Test del **contenido** del fichero, no del worker en ejecución: jsdom no
// implementa service workers. Vite copia `public/` tal cual a `dist/`.
// Importamos el fuente con la API `?raw` de Vite (`vite/client` declara el
// tipo), sin tocar `node:fs`: el proyecto no tiene `@types/node`.
import source from "../../public/sw.js?raw";

describe("public/sw.js", () => {
  it("declara el handler `push`", () => {
    expect(source).toContain("addEventListener('push'");
  });

  it("declara el handler `notificationclick`", () => {
    expect(source).toContain("addEventListener('notificationclick'");
  });

  it("no registra un handler `fetch` (no debe interceptar peticiones)", () => {
    expect(source).not.toContain("addEventListener('fetch'");
  });
});
