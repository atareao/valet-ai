# Tasks: oximeal-search-semantics

Issue: [#361](https://github.com/atareao/valet-ai/issues/361)

## 1. RED (tests que fallan)

### 1.1 Servicio (`src/services/oximeal.rs`)
- [x] 1.1.1 Test (`wiremock`): query de un término → **una** sola petición, sin cambio de comportamiento.
- [x] 1.1.2 Test: query de tres términos → una petición por término, todas con el mismo `category`.
- [x] 1.1.3 Test: la receta que coincide con **todos** los términos va en primer lugar.
- [x] 1.1.4 Test: sin coincidencia completa, orden por número de coincidencias descendente.
- [x] 1.1.5 Test: sin ids duplicados y acotado a 10.
- [x] 1.1.6 Test: más términos que el tope → se usan los primeros, sin fallar.
- [x] 1.1.7 `cargo test` → RED.
- [x] 1.1.8 Test: la respuesta de un término con el mismo id repetido cuenta como una sola coincidencia.

### 1.2 Herramienta (`src/tools/oximeal.rs`)
- [x] 1.2.1 Test: el esquema de `query` menciona «subcadena contigua» y la combinación de términos.
- [x] 1.2.2 Test: búsqueda sin resultados → mensaje con la semántica y una sugerencia.
- [x] 1.2.3 `cargo test` → RED.

### 1.3 Migración (`SKILL_MENUS_PROMPT`)
- [x] 1.3.1 Test: la migración actualiza el texto por defecto y conserva `# SKILL ACTIVA: MENUS`.
- [x] 1.3.2 Test: la migración NO pisa un texto editado por el usuario.
- [x] 1.3.3 `cargo test` → RED.

## 2. GREEN (implementación mínima)
- [x] 2.1 Implementar la combinación de términos en `search_recipes` (peticiones en paralelo + tope de términos).
- [x] 2.2 Ampliar la descripción del parámetro `query` en el esquema de la herramienta.
- [x] 2.3 Aviso accionable cuando el resultado está vacío.
- [x] 2.4 Migración nueva con guarda byte a byte.
- [x] 2.5 `cargo test` verde y `cargo check` sin errores.

## 3. REFACTOR
- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 `npx vitest run` en `frontend/` (sin cambios esperados).
- [x] 3.3 `openspec validate --all --strict` limpio.

## 4. CIERRE
- [x] 4.1 Actualizar `docs/oximeal-integration.md`.
- [x] 4.2 Auditoría `@rust-reviewer` sin bloqueantes.
- [x] 4.3 PR a `development` (gitmoji) y release a `main`.
- [x] 4.4 Despliegue en `co1` y E2E: planificar el menú con una búsqueda de varias palabras.
- [x] 4.5 `openspec archive`, índice de `plans/`, issue #361 cerrado.
