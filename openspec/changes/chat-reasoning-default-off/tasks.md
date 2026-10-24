# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — backend

- [x] 1.1 Test que falla: el default de razonamiento del rol chat es `off` cuando la clave `GENERATION_CHAT_REASONING` falta.
- [x] 1.2 Test que falla: una base recién migrada deja `GENERATION_CHAT_REASONING = off` (la migración corrige el vacío heredado).
- [x] 1.3 Test que falla: un valor explícito distinto (p. ej. `high`) se respeta tras la migración.
- [x] 1.4 Test que falla: un vacío explícito (`""`) sigue significando `None` (no se envía el campo `reasoning`).

## 2. RED — frontend

- [x] 2.1 Test que falla: el bloque de chat del SettingsDialog muestra `off` cuando `GENERATION_CHAT_REASONING` falta (y `default` cuando llega vacío explícito).
- [x] 2.2 Test que falla: al guardar sin tocar el campo del chat, se envía `off`.

## 3. GREEN — backend

- [x] 3.1 `GenerationRole::Chat` default de razonamiento: de `""` a `"off"` (`src/generation.rs`).
- [x] 3.2 Nueva migración (`migrations/<timestamp posterior al último>_chat_reasoning_off.sql`) con `UPDATE settings SET value='off', updated_at=datetime('now') WHERE key='GENERATION_CHAT_REASONING' AND value='';`.
- [x] 3.3 Actualizar los tests existentes que fijan el default vacío de `GENERATION_CHAT_REASONING`.

## 4. GREEN — frontend

- [x] 4.1 `SettingsDialog`: el fallback del chat pasa de `""` a `"off"`; la **carga** usa `??` (ausente⇒`off`, `""` explícito⇒`default`) y el guardado `??` (sin tocar). Los otros cuatro roles, intactos.

## 5. REFACTOR

- [x] 5.1 `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- [x] 5.2 `npx tsc --noEmit`, `npm run lint`, `npx vitest run`.
- [x] 5.3 Documentar el cambio de default — **no procede**: ni `README.md` ni `README.es.md` documentan los mandos de generación (`GENERATION_*`).

## 6. Verificación y cierre

- [ ] 6.1 Verificar en un despliegue de trabajo que el chat no emite tokens de razonamiento con un modelo razonador y el default nuevo.
- [ ] 6.2 `openspec archive chat-reasoning-default-off`.
