# PLAN-023 — OxiMeal: recetas con instrucciones y tiempo de preparación

- **Issue de entrada:** [#334](https://github.com/atareao/valet-ai/issues/334) (asignado a `atareao`)
- **Change de OpenSpec:** `openspec/changes/oximeal-recipe-instructions/`
- **Estado:** implementado y auditado; pendiente de verificación E2E y archivo

## Objetivo

Que Valet pueda **crear recetas completas en OxiMeal**, incluyendo sus **instrucciones de elaboración** (`instructions`) y su **tiempo de preparación** (`prep_time_minutes`), y no solo nombre, categoría e ingredientes.

## Diagnóstico (verificado)

El fallo es de **Valet**, no de OxiMeal:

- OxiMeal ya acepta `instructions: Option<String>` y `prep_time_minutes: Option<i64>` en su `RecipeInput` (`backend/src/models/recipe.rs`) y `POST /api/agent/recipes` los deserializa.
- En Valet, `parse_recipe` (`src/tools/oximeal.rs`) hacía passthrough del objeto `recipe` (no filtraba nada), pero el **JSON Schema** del parámetro `recipe` y el prompt `SKILL_MENUS_PROMPT` solo documentaban `name`, `category` e `ingredients`.
- Evidencia en producción (`co1`, 2026-11-07): 14 recetas creadas vía `create_recipe` con `instructions` vacío en la BD real de OxiMeal.

## Alcance ejecutado

1. Esquema del parámetro `recipe` de `create_recipe` (`src/tools/oximeal.rs`): documenta `instructions` y `prep_time_minutes` (y `servings`, `diet_type`, `source_url`, `tags`).
2. `parse_recipe`: passthrough confirmado y documentado; sigue exigiendo `name`/`category`/`ingredients` no vacíos.
3. Fragmento `SKILL_MENUS_PROMPT`: pasa a pedir `instructions` y `prep_time_minutes`.
4. Migración nueva `migrations/20261109000001_oximeal_recipe_instructions.sql`: aditiva, idempotente, con guarda byte a byte del fragmento anterior.
5. Tests: servicio (passthrough con wiremock), herramienta (esquema y validación) y prompt/migración.

## Decisiones

- **D1** — El arreglo es de Valet (contrato/documentación). OxiMeal no se toca.
- **D2** — Passthrough sin transformación: el objeto `recipe` viaja tal cual.
- **D3** — `update_recipe` fuera de alcance: la Agent API de OxiMeal no expone `PUT/PATCH`.
- **D4** — Migración de prompt con guarda byte a byte (precedente `20261104000001`).
- **D5** — Sin backfill de las recetas existentes.

## Verificación

- `cargo test` (suite completa): verde. `cargo clippy --all-targets -- -D warnings`: limpio. `cargo fmt --check`: limpio.
- Auditoría `rust-reviewer`: APROBADO (sin hallazgos de código).
- **Pendiente:** E2E contra la instancia real de OxiMeal en `co1`.

## Follow-ups

- Las 14 recetas ya creadas siguen sin instrucciones: recrearlas por chat o añadir `PUT/PATCH` a la Agent API de OxiMeal (change futuro).
- Archivar el change (`openspec archive`) y crear `docs/oximeal-recipe-instructions.md` tras la E2E.