# OxiMeal: recetas con instrucciones y tiempo de preparación

## Qué es

La herramienta `oximeal` (skill `menus`) creaba recetas en OxiMeal con **nombre, categoría e ingredientes**, pero sin **instrucciones de elaboración** ni **tiempo de preparación**. Este cambio documenta y expone esos dos campos opcionales del `RecipeInput` de OxiMeal para que el modelo los rellene y las recetas se guarden completas.

Referencia: issue [#334](https://github.com/atareao/valet-ai/issues/334), change `oximeal-recipe-instructions`, PRs #335/#336, release **v0.33.0**.

## Diagnóstico

El fallo era de **Valet**, no de OxiMeal:

- OxiMeal ya acepta `instructions: Option<String>` y `prep_time_minutes: Option<i64>` en su `RecipeInput` (`/data/rust/oximeal`, `backend/src/models/recipe.rs`) y `POST /api/agent/recipes` los deserializa sin descartarlos.
- En Valet, `parse_recipe` (`src/tools/oximeal.rs`) ya hacía *passthrough* del objeto `recipe` (no filtraba nada). El problema era **documental**: el JSON Schema del parámetro `recipe` y el fragmento `SKILL_MENUS_PROMPT` solo nombraban `name`, `category` e `ingredients`, de modo que el modelo no sabía que podía enviar `instructions`/`prep_time_minutes`.

Evidencia en producción (`co1`, 2026-11-07): 14 recetas creadas vía `create_recipe` quedaron con `instructions` vacío.

## Ficheros implicados

- `src/tools/oximeal.rs` — el esquema de `recipe` en `create_recipe` documenta `instructions` y `prep_time_minutes` (además de `servings`, `diet_type`, `source_url`, `tags`); `parse_recipe` sigue en passthrough.
- `src/services/oximeal.rs` — tests que confirman que los campos opcionales viajan y que se omiten si no vienen.
- `migrations/20261109000001_oximeal_recipe_instructions.sql` — actualiza el valor de `SKILL_MENUS_PROMPT`.
- `src/orchestrator/skills.rs` — declara la skill `menus` (solo `prompt_key`/`prompt_heading`; el **texto** del fragmento vive en migraciones, no compilado).

## Cómo funciona

1. El router activa la skill `menus`.
2. El fragmento `SKILL_MENUS_PROMPT` pide al modelo que, con `create_recipe`, incluya `instructions` y `prep_time_minutes` además de `name`/`category`/`ingredients`.
3. La herramienta valida los obligatorios **sin salir a la red** y acepta los opcionales tal cual.
4. El permiso de `create_recipe` es **`Notify`** (aviso, no bloquea).
5. El servicio hace passthrough y envía el `RecipeInput` entero a `POST /api/agent/recipes`.
6. OxiMeal valida y persiste → la receta aparece con preparación y tiempo.

`instructions` y `prep_time_minutes` son **opcionales**: si el modelo no los da, la receta se crea igual (incompleta).

## Configuración

Sin ajustes nuevos. Todo se apoya en los ajustes ya existentes de OxiMeal (`oximeal_base_url`, `oximeal_issuer_url`, `oximeal_client_id`, `oximeal_client_secret`).

## Cómo habilitar / desplegar

No requiere pasos manuales: la migración se ejecuta sola al arrancar el contenedor.

- Desplegado en `co1` como **v0.33.0** (Dockge). Al recrear el stack, la migración actualiza `SKILL_MENUS_PROMPT`.

## Decisiones

- **D1** — El arreglo es de Valet (contrato/documentación). OxiMeal no se toca.
- **D2** — Passthrough sin transformación.
- **D3** — `update_recipe` fuera de alcance: la Agent API de OxiMeal no expone `PUT/PATCH`.
- **D4** — La migración usa guarda **byte a byte**: si el fragmento fue editado por el usuario, no se pisa. Precedente `20261104000001`.
- **D5** — Sin backfill de las recetas existentes.

## Follow-ups

- Las 14 recetas creadas antes de v0.33.0 siguen sin instrucciones. Para recuperarlas: recrearlas por chat, o abrir un change que añada `update_recipe` a la Agent API de OxiMeal.
- Verificación E2E: confirmada por el usuario en el chat real (receta guardada con instrucciones y tiempo).

## Referencias

- Change OpenSpec: `openspec/changes/archive/*-oximeal-recipe-instructions/`
- Plan: `plans/PLAN-023-ARCHIVED.md`
- Integración base: `docs/oximeal-integration.md`
