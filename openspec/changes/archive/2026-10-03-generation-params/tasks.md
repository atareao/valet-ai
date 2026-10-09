# Tasks: Parámetros de generación por rol

## Bloque 1 — Contrato (tipos y serialización)
- [x] 1.1 RED/GREEN: `ReasoningSpec`, `ReasoningEffort` y `ResponseFormat` con su serialización
  exacta (`Off` → `{enabled:false}`, `Effort` → `{effort:"<nivel>"}`, `JsonObject` →
  `{type:"json_object"}`).
- [x] 1.2 RED/GREEN: `ChatRequest` gana `reasoning` y `response_format`, ambos `None` por defecto y
  omitidos cuando son `None`.

## Bloque 2 — Provider OpenRouter
- [x] 2.1 RED/GREEN: `chat()` reenvía `reasoning` y `response_format` con la forma exacta, y los
  omite cuando son `None`.
- [x] 2.2 RED/GREEN: `chat_stream()` reenvía `reasoning` idénticamente y respeta `None`.
- [x] 2.3 RED/GREEN: `OllamaProvider` ignora ambos campos sin fallar.

## Bloque 3 — Settings (migración + lectura)
- [x] 3.1 RED/GREEN: migración que siembra las doce claves `GENERATION_*` con sus defaults,
  respetando cualquier valor no vacío ya existente.
- [x] 3.2 RED/GREEN: helper de lectura y parseo de los parámetros de un rol (temperatura,
  razonamiento, tokens) con fallback al default y warning ante valor inválido; leído en cada
  llamada, no al arrancar.

## Bloque 4 — Workers
- [x] 4.1 RED/GREEN: `CollapseWorker` lee sus tres parámetros de `settings` en cada llamada
  (default: 0.2 / off / 1024).
- [x] 4.2 RED/GREEN: la extracción de fichas lee sus tres parámetros (default: 0.3 / off / 1024).
- [x] 4.3 RED/GREEN: consolidación y compresión envían `response_format: json_object`
  incondicional, y usan `GENERATION_SEMANTIC_*` (default: 0.1 / low / 2048).

## Bloque 5 — Orquestador (chat)
- [x] 5.1 RED/GREEN: el chat lee `GENERATION_CHAT_*` en cada turno (default: 0.7 / vacío / 4096) y
  los aplica al `ChatRequest` de `process_message_stream()` (el camino que usa `chat_stream()`).

## Bloque 6 — Frontend: pestaña «Generación»
- [x] 6.1 RED/GREEN: tipos de las doce claves y los campos de la tab (temperatura numérica,
  razonamiento selector, tokens numéricos) con las opciones `default/off/minimal/low/medium/high/xhigh/max`.
- [x] 6.2 RED/GREEN: carga desde `GET /settings` y guardado con `updateSettings` sin perder las
  demás claves; mensaje "Ajustes guardados".
- [x] 6.3 Montar la pestaña «Generación» (cuatro bloques) en `SettingsDialog`.

## Bloque 7 — Verificación y limpieza
- [x] 7.1 `cargo fmt --check` limpio.
- [x] 7.2 `cargo clippy --all-targets -- -D warnings` limpio.
- [x] 7.3 `cargo test` completo en verde, sin regresiones.
- [x] 7.4 `cd frontend && npx tsc --noEmit && npx vitest run` en verde.
- [x] 7.5 `cd frontend && npm run lint:ci` limpio.
- [x] 7.6 `openspec validate generation-params --strict` válido.
