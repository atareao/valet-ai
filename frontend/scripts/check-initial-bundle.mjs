#!/usr/bin/env node
/**
 * Comprobación determinista del bundle inicial (change `lazy-app-bundle`).
 *
 * Ejecutado en `postbuild` (por tanto en `npm run build`, `just frontend-check`
 * y CI). Analiza el grafo de importaciones **estáticas** del entrypoint HTML y
 * falla si:
 *
 *   1. algún chunk del grafo inicial pertenece a un vendor que debe quedar
 *      fuera (`vendor-antd`, `vendor-antd-icons`, `vendor-charts`,
 *      `vendor-markdown`, `vendor-leaflet`, `vendor-router`) o a la app
 *      autenticada (`AuthenticatedApp`);
 *   2. el tamaño gzip total del grafo inicial supera el presupuesto (70 kB).
 *      El grafo real pesa ~47 kB, así que 70 kB deja un margen estrecho y
 *      detecta fugas de ~100 kB que un umbral de 150 kB dejaría pasar;
 *   3. no existe un chunk dinámico `AuthenticatedApp-*.js` (guarda de
 *      no-vacuidad: si la app no se hubiera separado, esto debe fallar, no
 *      pasar por vacío).
 *
 * ESM Node puro, sin dependencias externas.
 */

import { readFileSync, existsSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";
import { dirname, join, basename } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const DIST = join(ROOT, "dist");
const MANIFEST = join(DIST, ".vite", "manifest.json");

/** Presupuesto de gzip para el grafo inicial. */
const BUDGET_BYTES = 70 * 1024; // 71680

/**
 * Prefijos de chunk que NO pueden aparecer en el bundle inicial.
 *
 * `vendor-router` no tiene grupo deliberado en `vite.config.ts`: los módulos de
 * react-router solo son alcanzables desde `AuthenticatedApp` (entry dinámico),
 * así que deben quedar dentro del chunk diferido. Se mantiene aquí como guarda
 * de regresión: si algún día aparece un chunk con ese nombre en el grafo
 * inicial, es que el router se ha filtrado al bundle.
 */
const FORBIDDEN = [
  "vendor-antd",
  "vendor-antd-icons",
  "vendor-charts",
  "vendor-markdown",
  "vendor-leaflet",
  "vendor-router",
  "AuthenticatedApp",
];

function fail(message) {
  console.error(`\n✖ check-initial-bundle: ${message}\n`);
  process.exit(1);
}

function formatKb(bytes) {
  return `${(bytes / 1024).toFixed(2)} kB`;
}

/** Tamaño gzip real del fichero referenciado por una entrada del manifiesto. */
function gzipSizeOf(file) {
  const abs = join(DIST, file);
  if (!existsSync(abs)) {
    fail(`falta el fichero declarado en el manifiesto: ${file}`);
  }
  return gzipSync(readFileSync(abs)).length;
}

function main() {
  if (!existsSync(MANIFEST)) {
    fail(
      `no existe ${MANIFEST}. Ejecuta \`vite build\` con \`build.manifest: true\`.`,
    );
  }

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));

  // 1. Localizar el entrypoint HTML (`isEntry: true`).
  const entry = Object.values(manifest).find((c) => c && c.isEntry === true);
  if (!entry) {
    fail("el manifiesto no declara ninguna entrada con `isEntry: true`.");
  }

  // 2. Recorrer el grafo de importaciones estáticas (`imports`) desde la entrada.
  const visited = new Set();
  const initialFiles = [];
  const queue = [entry];
  while (queue.length > 0) {
    const chunk = queue.shift();
    if (!chunk || !chunk.file || visited.has(chunk.file)) continue;
    visited.add(chunk.file);
    initialFiles.push(chunk);

    for (const importKey of chunk.imports ?? []) {
      const dep = manifest[importKey];
      if (dep) queue.push(dep);
    }
  }

  // 3. Vendors prohibidos en el grafo inicial.
  const offenders = [];
  for (const chunk of initialFiles) {
    const label = chunk.name ?? basename(chunk.file);
    if (FORBIDDEN.some((prefix) => label.startsWith(prefix))) {
      offenders.push(label);
    }
  }

  // 4. Tamaño gzip total del grafo inicial. Se calcula una sola vez por
  // fichero y se reutiliza tanto para el resumen como para la suma.
  const sizes = new Map();
  let totalBytes = 0;
  for (const chunk of initialFiles) {
    const size = gzipSizeOf(chunk.file);
    sizes.set(chunk.file, size);
    totalBytes += size;
  }

  // 5. Guarda de no-vacuidad: existe un chunk dinámico `AuthenticatedApp-*.js`.
  const dynamicApp = Object.values(manifest).some(
    (c) =>
      c &&
      c.isDynamicEntry === true &&
      /(^|\/)AuthenticatedApp-[^/]+\.js$/.test(c.file),
  );
  // También aceptamos el nombre de chunk por si el fichero no llevara hash.
  const dynamicAppByName = Object.values(manifest).some(
    (c) => c && c.isDynamicEntry === true && c.name === "AuthenticatedApp",
  );
  const hasLazyApp = dynamicApp || dynamicAppByName;

  // 6. Resumen legible (siempre).
  console.log("check-initial-bundle: grafo inicial");
  for (const chunk of initialFiles) {
    const label = chunk.name ?? basename(chunk.file);
    console.log(`  • ${label} — ${formatKb(sizes.get(chunk.file))}`);
  }
  console.log(
    `  total gzip inicial: ${formatKb(totalBytes)} (presupuesto ${formatKb(BUDGET_BYTES)})`,
  );

  const problems = [];
  if (offenders.length > 0) {
    problems.push(
      `el grafo inicial incluye vendors/app autenticada prohibidos: ${[
        ...new Set(offenders),
      ].join(", ")}`,
    );
  }
  if (totalBytes > BUDGET_BYTES) {
    problems.push(
      `el grafo inicial pesa ${formatKb(totalBytes)} gzip, supera el presupuesto de ${formatKb(BUDGET_BYTES)}`,
    );
  }
  if (!hasLazyApp) {
    problems.push(
      "no existe un chunk dinámico `AuthenticatedApp-*.js` (la app autenticada no está diferida)",
    );
  }

  if (problems.length > 0) {
    fail(problems.map((p) => `- ${p}`).join("\n"));
  }

  console.log("✔ check-initial-bundle: OK");
  process.exit(0);
}

main();
