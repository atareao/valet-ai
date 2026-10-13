# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

Comandos de verificación de este repo: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`, `openspec validate --all`.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + deltas de `db/repos` y `frontend` + design + tasks) y **esperar aprobación explícita** antes de tocar código. **No se escribe código ni tests antes de la aprobación.**
- [x] 0.2 Resolver las **decisiones abiertas** del `design.md`: (a) la asimetría de `StatsRepo::summary` (que sigue filtrando `kind='chat'`) y (b) el default de `by_day` (30) frente al de la ventana (7). Propuesta: no tocarlas en este change y registrarlas como follow-up.

## 1. RED — `db/repos` (una fila por pareja origen, modelo)

- [x] 1.1 RED — test en `src/db/repos/stats.rs` (`#[cfg(test)]`): dos llamadas `consolidator` con modelos distintos producen **dos** filas, una por pareja (origen, modelo), con sus propias cifras. Hoy producen una sola fila por origen. **Pegar la salida en rojo en el informe.**
- [x] 1.2 RED — test: `BackgroundStats` expone `model: Option<String>` y las parejas reales lo llevan poblado.
- [x] 1.3 RED — test: un origen canónico sin filas sigue apareciendo con contadores a cero y `model: null`.
- [x] 1.4 RED — test: el orden es por `kind` canónico (`router`, `archivist`, `consolidator`, `collapse`, `timeline`) y, dentro de cada `kind`, por coste descendente.
- [x] 1.5 RED — test: `suspicious_calls` cuenta las filas con `status='success'` y `completion_tokens <= 1`, y **no** cuenta las que tienen `status='error'` (aunque tengan 1 token).
- [x] 1.6 RED — test: `last_call_at` es `MAX(created_at)` de la pareja y es `None` para un origen sin filas.
- [x] 1.7 RED — test: `days` acota la ventana y descarta las filas anteriores a `datetime('now', '-N days')`.

## 2. RED — `db/repos` (panel de modelos unificado)

- [x] 2.1 RED — test en `src/db/repos/stats.rs`: un modelo que solo aparece en llamadas de fondo (p. ej. `qwen/...` en `consolidator`) aparece en `by_model` con su coste agregado. Hoy, al filtrar `kind='chat'`, no aparece.
- [x] 2.2 RED — test: `by_model` agrega en una sola fila las llamadas del mismo modelo hechas en chat y en roles de fondo.
- [x] 2.3 RED — test: `by_model` recibe `days` y descarta las llamadas fuera de la ventana.

## 3. RED — handler y frontend

- [x] 3.1 RED — test del handler (`src/handlers/stats.rs`): `GET /api/stats/llm/background` y `GET /api/stats/llm/by-model` propagan el `days` recibido y usan **7** cuando no llega.
- [x] 3.2 RED — test de frontend (`npx vitest run`): la tarjeta «Procesos de fondo» renderiza una fila por pareja (rol, modelo), muestra la última llamada, marca las sospechosas y resalta los errores `> 0`.
- [x] 3.3 RED — test de frontend: el selector de ventana gobierna las tres consultas (diaria, roles y modelos) y parte de **7** días por defecto.
- [x] 3.4 RED — test de frontend: el panel de modelos agrega todos los orígenes y su título/etiqueta ya no dicen «solo chat».
- [x] 3.5 RED — test de frontend: la tarjeta de modelos incluye un modelo de un rol de fondo y las tarjetas de resumen y diaria siguen **sin** incluir los orígenes de fondo.

## 4. GREEN — implementación mínima

- [x] 4.1 GREEN — `src/db/repos/stats.rs`: `GROUP BY kind, model`, relleno de ceros por origen canónico, campos `model`, `suspicious_calls` (umbral en constante con nombre) y `last_call_at`, orden por `kind` canónico y coste descendente.
- [x] 4.2 GREEN — `src/db/repos/stats.rs`: retirar el `WHERE kind='chat'` de `by_model` y agrupar por modelo sobre todos los orígenes, con `days`.
- [x] 4.3 GREEN — `src/handlers/stats.rs`: propagar `days` a `background_summary` y a `by_model` con el patrón `Query<DayParams>` y default 7. **No tocar** `tools_summary` ni la consulta de `tool_calls`.
- [x] 4.4 GREEN — frontend: fila por (rol, modelo) con última llamada, marca de sospechosas y errores resaltados; selector de ventana que gobierna los tres paneles (default 7); título/etiqueta del panel de modelos acordes.
- [x] 4.5 GREEN — `cargo test` y `npx vitest run` en verde.

## 5. REFACTOR

- [x] 5.1 REFACTOR — limpiar artefactos en cascada (campos, constantes o tests que queden sin uso al cambiar la firma de `background_summary` y `by_model`).
- [x] 5.2 REFACTOR — `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings nuevos.
- [x] 5.3 REFACTOR — `npx tsc --noEmit` sin errores.

## 6. Verificación y cierre

- [x] 6.1 `cargo test` (suite completa) en verde.
- [x] 6.2 `cargo clippy --all-targets -- -D warnings` en verde.
- [x] 6.3 `cargo fmt --check` en verde.
- [x] 6.4 `npx vitest run` en verde.
- [x] 6.5 `npx tsc --noEmit` en verde.
- [x] 6.6 `openspec validate --all` en verde.
- [ ] 6.7 Medir en producción tras desplegar: la fila del consolidador ya no funde los tres modelos (`GROUP BY kind, model`) y la avería de `deepseek` aparece en `suspicious_calls`. Dejar constancia del antes y el después.
- [ ] 6.8 Parar y esperar aprobación del usuario. Aprobar el archive (`openspec archive stats-role-model-view`) en un PR aparte tras el merge.
