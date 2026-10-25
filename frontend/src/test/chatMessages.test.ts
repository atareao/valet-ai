import { describe, it, expect } from "vitest";
import {
  normalizeServerTimestamp,
  sortMessages,
  upsertMessage,
  upsertMessages,
} from "../utils/chatMessages";
import type { Message } from "../types";

/**
 * Fase RED de `chat-cross-device-sync` y de `chat-pending-flag-leak`.
 *
 * Contrato (spec `frontend`): la lista de mensajes se fusiona por **upsert de
 * id** (idempotente) y se ordena por la pareja `(created_at, id)` usando
 * **siempre la marca temporal del servidor**; un mensaje optimista permanece al
 * final hasta que llegue su fila canónica.
 *
 * Desde `chat-pending-flag-leak` la condición de «optimista» viaja **en el
 * propio mensaje** (`pending?: boolean`, campo interno del cliente), no en un
 * `Set` de identificadores aparte: `sortMessages`, `upsertMessage` y
 * `upsertMessages` ya **no** reciben ese `Set`. Una fila canónica nunca puede
 * conservar la marca, porque al fusionarla la escritura entrante (sin marca) la
 * retira, y una escritura optimista sobre una fila ya canónica se descarta.
 */

const msg = (
  id: string,
  created_at: string,
  extra: Partial<Message> = {},
): Message => ({
  id,
  role: "user",
  content: id,
  created_at,
  ...extra,
});

describe("normalizeServerTimestamp", () => {
  it("adopta el instante del servidor sin reinterpretarlo como hora local", () => {
    // El crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`, UTC) se normaliza a ISO-8601
    // UTC: `new Date()` sobre la cadena cruda lo leería como hora **local**.
    expect(normalizeServerTimestamp("2026-10-24 10:00:01")).toBe(
      "2026-10-24T10:00:01.000Z",
    );
  });

  it("respeta un ISO ya normalizado", () => {
    expect(normalizeServerTimestamp("2026-10-24T10:00:01.000Z")).toBe(
      "2026-10-24T10:00:01.000Z",
    );
  });

  it("deja pasar tal cual lo que no parsea, sin inventar una fecha", () => {
    expect(normalizeServerTimestamp("no-es-fecha")).toBe("no-es-fecha");
  });
});

describe("upsertMessage — fusión idempotente por id", () => {
  it("inserta una fila nueva y sustituye la que ya tenía ese id", () => {
    const first = upsertMessage([], msg("m1", "2026-10-24 10:00:01"));
    expect(first.map((m) => m.id)).toEqual(["m1"]);

    const replaced = upsertMessage(
      first,
      msg("m1", "2026-10-24 10:00:01", { content: "canónico" }),
    );
    expect(replaced).toHaveLength(1);
    expect(replaced[0].content).toBe("canónico");
  });

  it("es idempotente: la misma fila dos veces no duplica", () => {
    const once = upsertMessage([], msg("m1", "2026-10-24 10:00:01"));
    const twice = upsertMessage(once, msg("m1", "2026-10-24 10:00:01"));

    expect(twice).toHaveLength(1);
    expect(twice).toEqual(once);
  });

  it("sustituye la burbuja optimista por su fila canónica conservando el id", () => {
    const optimista = msg("cli-1", "2026-10-24T12:00:00.000Z", {
      content: "Hola",
      pending: true,
    });

    const conOptimista = upsertMessage([], optimista);
    expect(conOptimista).toHaveLength(1);

    // Llega la fila canónica con el mismo id (el servidor respeta
    // `client_message_id`) y la marca temporal del servidor.
    const canonica = upsertMessage(
      conOptimista,
      msg("cli-1", "2026-10-24 12:00:03", { content: "Hola" }),
    );

    expect(canonica).toHaveLength(1);
    expect(canonica[0].id).toBe("cli-1");
    expect(canonica[0].created_at).toBe("2026-10-24T12:00:03.000Z");
  });

  it("la fila canónica retira la marca de optimista de la fila fusionada", () => {
    // Escenario «El estado de optimista no sobrevive a la fila canónica».
    const optimista = msg("cli-1", "2026-10-24T12:00:00.000Z", {
      content: "Hola",
      pending: true,
    });
    const conOptimista = upsertMessage([], optimista);
    expect(conOptimista[0].pending).toBe(true);

    const canonica = upsertMessage(
      conOptimista,
      msg("cli-1", "2026-10-24 12:00:03", { content: "Hola" }),
    );

    // La marca viaja en el mensaje: al fusionar la fila del servidor (sin
    // marca) la fusión la retira, y la fila vuelve a la pareja `(created_at, id)`.
    expect(canonica[0].pending).not.toBe(true);
    expect(canonica[0].created_at).toBe("2026-10-24T12:00:03.000Z");
  });

  it("la guarda se mantiene: una escritura optimista no pisa una fila canónica", () => {
    // design D2: si el id ya existe como fila **canónica** (sin marca) y llega
    // una escritura optimista, la fusión la descarta en vez de sustituir la
    // marca temporal del servidor por la del reloj del navegador.
    const lista = upsertMessage(
      [],
      msg("a1", "2026-10-24 12:00:05", { role: "assistant", content: "final" }),
    );

    const resultado = upsertMessage(
      lista,
      msg("a1", "2000-01-01T00:00:00.000Z", {
        role: "assistant",
        content: "parcial",
        pending: true,
      }),
    );

    expect(resultado).toHaveLength(1);
    expect(resultado[0].created_at).toBe("2026-10-24T12:00:05.000Z");
    expect(resultado[0].content).toBe("final");
    expect(resultado[0].pending).not.toBe(true);
  });

  it("es idempotente con la marca en el mensaje: la misma fila dos veces no cambia", () => {
    const fila = msg("cli-1", "2026-10-24T12:00:00.000Z", {
      content: "Hola",
      pending: true,
    });

    const once = upsertMessage([], fila);
    const twice = upsertMessage(once, fila);

    expect(twice).toHaveLength(1);
    expect(twice).toEqual(once);
  });

  it("no muta la lista de entrada", () => {
    const original = [msg("m1", "2026-10-24 10:00:01")];
    const snapshot = [...original];

    upsertMessage(original, msg("m2", "2026-10-24 10:00:02"));

    expect(original).toEqual(snapshot);
  });
});

describe("upsertMessages — fusión de un lote con un solo reordenado", () => {
  it("fusiona el lote entero y ordena una sola vez", () => {
    // Un replay de N mensajes no debe reordenar N veces (O(N² log N)): el lote
    // entra con un `Map` por id y un único `sortMessages` al final.
    const result = upsertMessages(
      [msg("m1", "2026-10-24 10:00:01")],
      [
        msg("m3", "2026-10-24 10:00:03"),
        msg("m2", "2026-10-24 10:00:02"),
        msg("m1", "2026-10-24 10:00:01", { content: "actualizado" }),
      ],
    );

    expect(result.map((m) => m.id)).toEqual(["m1", "m2", "m3"]);
    expect(result[0].content).toBe("actualizado");
  });

  it("es idempotente, sustituye por id y no duplica el mismo id repetido", () => {
    const once = upsertMessages([], [
      msg("m1", "2026-10-24 10:00:01"),
      msg("m1", "2026-10-24 10:00:01", { content: "canónico" }),
    ]);

    expect(once).toHaveLength(1);
    expect(once[0].content).toBe("canónico");
  });

  it("mantiene los mensajes optimistas al final del lote", () => {
    const result = upsertMessages(
      [msg("m1", "2026-10-24 10:00:01")],
      [
        msg("cli-1", "2000-01-01T00:00:00.000Z", { pending: true }),
        msg("m2", "2026-10-24 10:00:02"),
      ],
    );

    expect(result.map((m) => m.id)).toEqual(["m1", "m2", "cli-1"]);
  });

  it("no muta la lista de entrada", () => {
    const original = [msg("m1", "2026-10-24 10:00:01")];
    const snapshot = [...original];

    upsertMessages(original, [msg("m2", "2026-10-24 10:00:02")]);

    expect(original).toEqual(snapshot);
  });

  it("la fila canónica de un lote retira la marca de optimista de la fila fusionada", () => {
    // Hueco de cobertura: la ruta del historial / lotes (`upsertMessages`) debe
    // cumplir la retirada de la marca (design D3) igual que el upsert suelto.
    const conOptimista = [msg("cli-1", "2000-01-01T00:00:00.000Z", { content: "Hola", pending: true })];

    // La fila canónica llega dentro de un lote, con el mismo id, la marca
    // temporal del servidor y **sin** la marca de optimista.
    const result = upsertMessages(conOptimista, [
      msg("cli-1", "2026-10-24 12:00:03", { content: "Hola" }),
    ]);

    expect(result.filter((m) => m.id === "cli-1")).toHaveLength(1);
    expect(result[0].created_at).toBe("2026-10-24T12:00:03.000Z");
    expect(result[0].pending).not.toBe(true);
  });

  it("un lote con mezcla deja la lista ordenada por (created_at, id) sin marca pendiente", () => {
    // El lote trae el canónico de la optimista `cli-1` y otro canónico normal:
    // todas las filas quedan ordenadas por la pareja del servidor, ninguna
    // conserva la marca.
    const result = upsertMessages(
      [
        msg("m1", "2026-10-24 10:00:01"),
        msg("cli-1", "2000-01-01T00:00:00.000Z", { content: "Hola", pending: true }),
      ],
      [
        msg("cli-1", "2026-10-24 10:00:03", { content: "Hola" }),
        msg("m2", "2026-10-24 10:00:02"),
      ],
    );

    expect(result.map((m) => m.id)).toEqual(["m1", "m2", "cli-1"]);
    expect(result.some((m) => m.pending === true)).toBe(false);
  });
});

describe("sortMessages — orden por la pareja (created_at, id)", () => {
  it("ordena por la marca temporal del servidor y desempata por id", () => {
    const list = [
      msg("b", "2026-10-24 10:00:02"),
      msg("c", "2026-10-24 10:00:01"),
      msg("a", "2026-10-24 10:00:02"),
    ];

    expect(sortMessages(list).map((m) => m.id)).toEqual(["c", "a", "b"]);
  });

  it("a igualdad de instante el orden es determinista, no el de llegada", () => {
    const timestamp = "2026-10-24 10:00:02";
    const uno = sortMessages([msg("b", timestamp), msg("a", timestamp)]);
    const otro = sortMessages([msg("a", timestamp), msg("b", timestamp)]);

    expect(uno.map((m) => m.id)).toEqual(["a", "b"]);
    expect(otro.map((m) => m.id)).toEqual(uno.map((m) => m.id));
  });

  it("coloca los mensajes optimistas al final hasta que llega su fila canónica", () => {
    const list = [
      // La hora local del navegador puede ser anterior a la del servidor: el
      // mensaje optimista no debe colarse al principio de la lista.
      msg("cli-1", "2000-01-01T00:00:00.000Z", { pending: true }),
      msg("m1", "2026-10-24 10:00:01"),
    ];

    expect(sortMessages(list).map((m) => m.id)).toEqual(["m1", "cli-1"]);
  });

  it("lee la marca de optimista del propio mensaje y la respeta al retirarla", () => {
    // Escenario (spec): un mensaje con hora del navegador antigua y marcado como
    // optimista va al FINAL; al retirarle la marca vuelve a su posición natural
    // por `(created_at, id)`.
    const conMarca = sortMessages([
      msg("cli-1", "2000-01-01T00:00:00.000Z", { pending: true }),
      msg("m1", "2026-10-24 10:00:01"),
    ]);
    expect(conMarca.map((m) => m.id)).toEqual(["m1", "cli-1"]);

    const sinMarca = sortMessages([
      msg("cli-1", "2000-01-01T00:00:00.000Z"),
      msg("m1", "2026-10-24 10:00:01"),
    ]);
    expect(sinMarca.map((m) => m.id)).toEqual(["cli-1", "m1"]);
  });

  it("mezcla ISO y crudo de SQLite por instante, no por texto", () => {
    // Comparar como texto daría "2026-10-24T…" ('T' > ' ') delante del crudo.
    const list = [
      msg("iso", "2026-10-24T10:00:05.000Z"),
      msg("crudo", "2026-10-24 10:00:04"),
    ];

    expect(sortMessages(list).map((m) => m.id)).toEqual(["crudo", "iso"]);
  });
});
