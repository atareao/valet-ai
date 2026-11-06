#!/usr/bin/env node
/**
 * Puerta de deprecaciones de antd (etapa A del change `frontend-react19-antd6`).
 *
 * `@ant-design/cli lint` devuelve **exit 0 aunque haya deprecaciones**, así que
 * el CLI por sí solo no sirve como puerta: hay que leer su JSON y decidir aquí.
 *
 * Reglas de la puerta:
 *
 *   1. Solo cuentan las incidencias con `rule === "deprecated"`. Las de
 *      `performance`, `a11y` y `usage` se ignoran a propósito: hoy arrastramos
 *      un `performance` preexistente (`virtual={false}` en `SettingsDialog.tsx`,
 *      ya presente en HEAD) y convertirlo en fallo rompería el árbol sin que
 *      nadie haya arreglado nada.
 *   2. Si hay deprecadas: se listan como `fichero:línea — mensaje` y se sale
 *      con código 1.
 *   3. Si el CLI no está instalado, no se puede ejecutar o su salida no es JSON
 *      válido, se sale con código 1: una puerta rota no debe pasar en silencio
 *      (fallar cerrado).
 *
 * ESM Node puro, sin dependencias externas.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative, sep } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");

/** Binario local del CLI (`@ant-design/cli` expone `antd`). */
const CLI = join(ROOT, "node_modules", ".bin", "antd");

/** Regla que hace fallar la puerta. */
const GATE_RULE = "deprecated";

function fail(message) {
  console.error(`\n✖ check-antd-deprecations: ${message}\n`);
  process.exit(1);
}

function formatPath(absolutePath) {
  const rel = relative(ROOT, absolutePath);
  return rel.split(sep).join("/");
}

/** Ficheros que el CLI recibe como entrada (`.ts` y `.tsx` bajo `src/`). */
function countSourceFiles(dir) {
  let total = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const abs = join(dir, entry.name);
    if (entry.isDirectory()) {
      total += countSourceFiles(abs);
    } else if (entry.isFile() && /\.tsx?$/.test(entry.name)) {
      total += 1;
    }
  }
  return total;
}

/** Ejecuta el CLI y devuelve su JSON ya parseado. */
function runCli() {
  if (!existsSync(CLI)) {
    fail(
      `no existe ${formatPath(CLI)}. Ejecuta \`npm ci\` (o \`npm install\`) para instalar @ant-design/cli.`,
    );
  }

  let stdout;
  try {
    stdout = execFileSync(CLI, ["lint", "src", "--format", "json"], {
      cwd: ROOT,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    // El CLI puede fallar por código de salida distinto de 0 (p. ej. 2 si no
    // encuentra la ruta) o por no existir el binario. En ninguno de los dos
    // casos la puerta debe pasar: se reporta el motivo real.
    const detail = error.stderr?.toString().trim() || error.message;
    fail(`el CLI @ant-design/cli falló al ejecutarse:\n${detail}`);
  }

  try {
    return JSON.parse(stdout);
  } catch {
    const preview = stdout.trim().slice(0, 500);
    fail(
      `la salida del CLI no es JSON parseable. Primeros 500 caracteres:\n${preview}`,
    );
  }
}

function main() {
  const result = runCli();

  if (!result || !Array.isArray(result.issues)) {
    fail(
      "la salida del CLI no contiene un array `issues`. ¿Ha cambiado el formato de @ant-design/cli?",
    );
  }

  const deprecated = result.issues.filter((issue) => issue.rule === GATE_RULE);
  const scanned = countSourceFiles(SRC);

  if (deprecated.length > 0) {
    const label = deprecated.length === 1 ? "deprecación" : "deprecaciones";
    console.error(`check-antd-deprecations: ${deprecated.length} ${label}`);
    for (const issue of deprecated) {
      const where = `${formatPath(issue.file)}:${issue.line}`;
      console.error(`  • ${where} — ${issue.message}`);
    }
    console.error(
      "\nSustituye cada API deprecada por la que indica el mensaje antes de continuar.\n",
    );
    process.exit(1);
  }

  console.log(
    `✔ check-antd-deprecations: OK — ${scanned} ficheros escaneados, 0 deprecaciones`,
  );
  process.exit(0);
}

main();