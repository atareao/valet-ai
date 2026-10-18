import { describe, it, expect } from "vitest";
import { parseSqliteTimestamp } from "../utils/sqliteTimestamp";

// Test de caracterización del helper movido a `src/utils/sqliteTimestamp.ts`
// (decisión D9 del design): su contrato es interpretar el crudo de SQLite
// (`"YYYY-MM-DD HH:MM:SS"`) como **UTC** y devolver `null` cuando no parsea.
describe("parseSqliteTimestamp", () => {
  it("interpreta el crudo de SQLite como UTC (instante exacto, sin depender de la TZ)", () => {
    // Determinista respecto a la zona horaria: se compara contra el epoch
    // esperado (`Date.UTC`), no contra un texto formateado.
    const instant = parseSqliteTimestamp("2026-10-13 19:12:33");
    expect(instant).not.toBeNull();
    expect(instant?.getTime()).toBe(Date.UTC(2026, 9, 13, 19, 12, 33));
  });

  it("devuelve null para null", () => {
    expect(parseSqliteTimestamp(null)).toBeNull();
  });

  it("devuelve null para una cadena no parseable", () => {
    expect(parseSqliteTimestamp("no-es-fecha")).toBeNull();
  });
});
