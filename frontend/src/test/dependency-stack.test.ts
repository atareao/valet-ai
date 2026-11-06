import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Guarda de contrato sobre `frontend/package.json` para la **ETAPA A** del
 * change `frontend-react19-antd6`: antd 6 sobre React 18.
 *
 * No cubre aquí lo que solo corresponde a la etapa B (React 19, react-leaflet 5,
 * chart.js 4.6): afirmarlo ahora dejaría este test en rojo durante toda la
 * etapa A por una razón ajena a su objeto.
 */
interface PackageJson {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

// `import.meta.url` no es `file:` bajo jsdom; vitest fija el cwd en `frontend/`.
const pkg = JSON.parse(
  readFileSync(join(process.cwd(), "package.json"), "utf8"),
) as PackageJson;

const deps = pkg.dependencies ?? {};
const devDeps = pkg.devDependencies ?? {};

/** Extrae la versión mínima (major.minor.patch) del rango declarado. */
function minVersion(range: string): string {
  const match = range.match(/(\d+)\.(\d+)\.(\d+)/);
  if (!match) throw new Error(`No se pudo interpretar el rango de versión «${range}»`);
  return `${match[1]}.${match[2]}.${match[3]}`;
}

/** Serie (major) de la versión mínima del rango declarado. */
function major(range: string): number {
  return Number(minVersion(range).split(".")[0]);
}

/** Compara dos versiones semánticas: > 0 si `a` es mayor que `b`. */
function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i += 1) {
    const diff = pa[i] - pb[i];
    if (diff !== 0) return diff;
  }
  return 0;
}

describe("stack de dependencias del frontend (etapa A)", () => {
  it("declara antd en la serie 6", () => {
    const range = deps["antd"];
    if (!range) throw new Error("antd no está declarado en dependencies");
    expect(major(range)).toBe(6);
  });

  it("declara @ant-design/icons con versión mínima >= 6.0.0", () => {
    const range = deps["@ant-design/icons"];
    if (!range) throw new Error("@ant-design/icons no está declarado en dependencies");
    expect(
      compareVersions(minVersion(range), "6.0.0"),
    ).toBeGreaterThanOrEqual(0);
  });

  it("no declara @ant-design/v5-patch-for-react-19", () => {
    expect(deps["@ant-design/v5-patch-for-react-19"]).toBeUndefined();
    expect(devDeps["@ant-design/v5-patch-for-react-19"]).toBeUndefined();
  });

  // Etapa B: react/react-dom/@types a la serie 19, react-leaflet 5, chart.js 4.6
});
