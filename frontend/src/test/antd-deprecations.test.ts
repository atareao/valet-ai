import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Guarda de regresión por análisis del código fuente (sin montar componentes):
 * el árbol `frontend/src` no debe contener ninguna API deprecada de antd 6.
 *
 * Deliberadamente **fuera** de esta guarda:
 *  - `bordered` (`Table bordered` sigue vigente en antd 6).
 *  - `direction` (se usa legítimamente en varios sitios, no solo `Space`).
 * Un escaneo de texto de esos dos daría falsos positivos; los cubre
 * `npx @ant-design/cli lint`, que no forma parte de esta tarea.
 */
interface Deprecation {
  /** Prop/componente deprecado tal cual aparece en el código. */
  token: string;
  /** API vigente en antd 6 que lo reemplaza. */
  replacement: string;
}

const DEPRECATIONS: readonly Deprecation[] = [
  { token: "tabPosition", replacement: "tabPlacement" },
  { token: "destroyOnClose", replacement: "destroyOnHidden" },
  { token: "maskClosable", replacement: "mask.closable" },
  { token: "dropdownRender", replacement: "popupRender" },
  { token: "bodyStyle", replacement: "styles.body" },
  { token: "headStyle", replacement: "styles.header" },
  { token: "overlayClassName", replacement: "classNames.root" },
  { token: "TabPane", replacement: "items" },
];

// `import.meta.url` no es `file:` bajo jsdom; vitest fija el cwd en `frontend/`.
/** Directorio raíz a escanear: `src/`. */
const srcDir = join(process.cwd(), "src");

/** Recolecta recursivamente los ficheros `.ts`/`.tsx` de `dir`. */
function collectSourceFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      // Se excluye el directorio `test/` para no encontrarse a sí mismo.
      if (entry.name === "test") continue;
      files.push(...collectSourceFiles(join(dir, entry.name)));
    } else if (/\.tsx?$/.test(entry.name)) {
      // Los tests colocados (`*.test.ts`, `*.test.tsx`) quedan fuera: un
      // comentario o un dato de prueba con una de las cadenas vigiladas no es
      // una deprecación en el código de la aplicación.
      if (/\.test\.tsx?$/.test(entry.name)) continue;
      files.push(join(dir, entry.name));
    }
  }
  return files;
}

describe("componentes vigentes de antd 6 (etapa A)", () => {
  it("no importa ni renderiza el componente List deprecado", () => {
    const violations: string[] = [];
    // El import puede ocupar varias líneas: se captura el bloque entre llaves
    // y se comprueban los identificadores uno a uno. Así `Listy` no coincide.
    const importPattern = /import\s*\{([\s\S]*?)\}\s*from\s*["']antd["']/g;
    // JSX inequívoco: `<List` seguido de algo que no sea un carácter de nombre
    // (`>`, espacio, salto, `/`, `.`). `<Listy` queda fuera.
    const jsxPattern = /<List(?![A-Za-z0-9_$])/g;

    for (const file of collectSourceFiles(srcDir)) {
      const source = readFileSync(file, "utf8");
      const lineAt = (index: number) =>
        source.slice(0, index).split("\n").length;
      const lineText = (index: number) =>
        source.slice(0, index).split("\n").pop() ?? "";

      for (const match of source.matchAll(importPattern)) {
        const identifiers = match[1]
          .split(",")
          .map((part) => part.trim().split(/\s+as\s+/)[0]?.trim())
          .filter(Boolean);
        if (identifiers.includes("List")) {
          violations.push(
            `${relative(process.cwd(), file)}:${lineAt(match.index ?? 0)}: «List» está deprecado en antd 6; usa «Listy»`,
          );
        }
      }

      for (const match of source.matchAll(jsxPattern)) {
        const trimmed = lineText(match.index ?? 0).trimStart();
        // Se ignoran las menciones en comentarios; el componente real no vive ahí.
        if (
          trimmed.startsWith("//") ||
          trimmed.startsWith("/*") ||
          trimmed.startsWith("*")
        ) {
          continue;
        }
        violations.push(
          `${relative(process.cwd(), file)}:${lineAt(match.index ?? 0)}: «List» está deprecado en antd 6; usa «Listy»`,
        );
      }
    }

    expect(
      violations,
      `Componentes deprecados de antd 6 encontrados:\n${violations.join("\n")}`,
    ).toEqual([]);
  });
});

describe("API vigente de antd 6 (etapa A)", () => {
  it("no usa props deprecadas de antd en el código fuente", () => {
    const violations: string[] = [];

    for (const file of collectSourceFiles(srcDir)) {
      const lines = readFileSync(file, "utf8").split("\n");
      lines.forEach((line, index) => {
        for (const { token, replacement } of DEPRECATIONS) {
          // Límite de palabra: son nombres de prop, no subcadenas de otra cosa.
          if (new RegExp(`\\b${token}\\b`).test(line)) {
            violations.push(
              `${relative(process.cwd(), file)}:${index + 1}: «${token}» está deprecado en antd 6; usa «${replacement}»`,
            );
          }
        }
      });
    }

    expect(
      violations,
      `Props deprecadas de antd 6 encontradas:\n${violations.join("\n")}`,
    ).toEqual([]);
  });
});
