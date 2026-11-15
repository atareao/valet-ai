import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Guarda de contrato sobre `frontend/package.json` para el stack de UI
 * completo: React 19 (con sus tipos), antd 6, react-leaflet 5 y chart.js 4.6.
 *
 * No vigila la cadena de herramientas (`jsdom`, `vitest`, `vite`, `eslint`):
 * cambia a otro ritmo y su contrato ya lo ejerce el CI al correr la suite.
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

describe("stack de dependencias de UI del frontend", () => {
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

  it("declara react en la serie 19", () => {
    const range = deps["react"];
    if (!range) throw new Error("react no está declarado en dependencies");
    expect(major(range)).toBe(19);
  });

  it("declara react-dom en la serie 19", () => {
    const range = deps["react-dom"];
    if (!range) throw new Error("react-dom no está declarado en dependencies");
    expect(major(range)).toBe(19);
  });

  it("declara @types/react en la serie 19", () => {
    const range = devDeps["@types/react"];
    if (!range) throw new Error("@types/react no está declarado en devDependencies");
    expect(major(range)).toBe(19);
  });

  it("declara @types/react-dom en la serie 19", () => {
    const range = devDeps["@types/react-dom"];
    if (!range) throw new Error("@types/react-dom no está declarado en devDependencies");
    expect(major(range)).toBe(19);
  });

  it("declara react-leaflet en la serie 5", () => {
    const range = deps["react-leaflet"];
    if (!range) throw new Error("react-leaflet no está declarado en dependencies");
    expect(major(range)).toBe(5);
  });

  it("declara chart.js en la serie 4.6", () => {
    const range = deps["chart.js"];
    if (!range) throw new Error("chart.js no está declarado en dependencies");
    const [majorPart, minorPart] = minVersion(range).split(".").map(Number);
    expect(majorPart).toBe(4);
    expect(minorPart).toBeGreaterThanOrEqual(6);
  });
});
