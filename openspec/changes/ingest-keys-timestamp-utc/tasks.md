# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

Comandos de verificación de este repo (solo frontend): `npx vitest run`, `npx tsc --noEmit`, `npm run lint`, `openspec validate --all`.

## 0. Aprobación (punto de parada del protocolo)

- [ ] 0.1 Presentar este change (proposal + delta de `frontend` + design + tasks) y **esperar aprobación explícita** antes de tocar código. **No se escribe código ni tests antes de la aprobación.**
- [ ] 0.2 Resolver la **decisión abierta** del `design.md` (D2): dónde vive `parseSqliteTimestamp` — (1) mover a un módulo compartido, (2) duplicar, o (3) importar desde `stats/`. **Bloquea el primer paso de GREEN.**

## 1. RED — el defecto se reproduce en un test

- [ ] 1.1 RED — test en `frontend/src/components/IngestKeysSection.test.tsx`: con `last_used_at = "2026-10-12 13:55:33"` (crudo de SQLite) la celda de «último uso» muestra ese instante **como UTC**, no como hora local. Hoy `new Date()` lo lee como hora local (o lo rechaza en WebKit). **Pegar la salida en rojo en el informe.**
- [ ] 1.2 RED — test: con `last_used_at` no parseable (p. ej. `"no es una fecha"`) la celda muestra `—` y **nunca** el valor crudo ni «Invalid Date». Hoy devuelve el valor crudo.
- [ ] 1.3 RED — test: con `last_used_at` nulo la celda muestra `—` (comportamiento que debe conservarse).
- [ ] 1.4 RED — test: `created_at` se interpreta con la misma regla que `last_used_at`.
- [ ] 1.5 RED — test: la marca no depende de la zona horaria (fijar `TZ`/mockear y comprobar que el instante mostrado es el mismo).

## 2. GREEN — implementación mínima

- [ ] 2.1 GREEN — aplicar la decisión de D2 sobre la ubicación del helper `parseSqliteTimestamp`.
- [ ] 2.2 GREEN — `frontend/src/components/IngestKeysSection.tsx`: `formatDate` usa el helper y devuelve `—` cuando no parsea (nunca el valor crudo).
- [ ] 2.3 GREEN — `npx vitest run` en verde.

## 3. REFACTOR

- [ ] 3.1 REFACTOR — limpiar artefactos en cascada (imports y comentarios que queden obsoletos en `IngestKeysSection.tsx`).
- [ ] 3.2 REFACTOR — `npx tsc --noEmit` sin errores.
- [ ] 3.3 REFACTOR — `npm run lint` sin warnings nuevos.

## 4. Verificación y cierre

- [ ] 4.1 `npx vitest run` (suite completa) en verde.
- [ ] 4.2 `npx tsc --noEmit` en verde.
- [ ] 4.3 `npm run lint` en verde.
- [ ] 4.4 `openspec validate --all` en verde.
- [ ] 4.5 Comprobar en el navegador (o con un test de `TZ`) que la hora mostrada ya no depende de la zona horaria de la máquina. Dejar constancia del antes y el después.
- [ ] 4.6 Parar y esperar aprobación del usuario. Aprobar el archive (`openspec archive ingest-keys-timestamp-utc`) en un PR aparte tras el merge.
