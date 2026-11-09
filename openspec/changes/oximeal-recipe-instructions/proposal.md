# Change: oximeal-recipe-instructions

## Why

La integración con OxiMeal (`oximeal`, skill `menus`) permite crear recetas, pero las crea **incompletas**: sin instrucciones de elaboración. El **2026-11-07** un turno real creó **14 recetas** con 14 llamadas a `create_recipe` y, verificado contra la BD real de OxiMeal (`oximeal.db`), las 14 quedaron con `instructions` **vacío**. El propio modelo lo confesó en su respuesta: *«Guardé las catorce con nombre, categoría e ingredientes — y ni una línea de preparación […] No hay campo de instrucciones en lo que la herramienta acepta.»*

El diagnóstico es claro y la culpa **no** es de OxiMeal: su `RecipeInput` (`backend/src/models/recipe.rs`) ya declara `instructions: Option<String>` (y `prep_time_minutes`, `servings`, `diet_type`, `source_url`, `tags`) y `POST /api/agent/recipes` lo deserializa sin descartarlo. El fallo está en **Valet**, y es de **contrato/documentación**:

- `fn parse_recipe` (`src/tools/oximeal.rs`) hace *passthrough* del objeto `recipe` (lo clona tal cual), así que **no filtra nada**.
- El **JSON Schema** del parámetro `recipe` solo documenta `name`, `category` e `ingredients` → el modelo nunca supo que podía enviar `instructions`.
- El prompt de la skill `SKILL_MENUS_PROMPT` insiste en lo mismo, sin nombrar instrucciones.

Este change cierra esa brecha de contrato para que Valet pueda **guardar recetas completas**. No requiere ningún cambio en OxiMeal.

## What Changes

- **Esquema de la herramienta `oximeal`** (`src/tools/oximeal.rs`): la propiedad `recipe` de `create_recipe` documenta los campos opcionales útiles de `RecipeInput` —como mínimo `instructions`, y también `prep_time_minutes`, `servings`, `diet_type`, `source_url` y `tags`— de modo que el modelo sepa que puede enviarlos.
- **Validación `parse_recipe`**: sigue aceptando el objeto tal cual; se documenta y prueba que `instructions` viaja **sin alterar**, y que se omite cuando no viene.
- **Prompt de la skill `menus`**: `SKILL_MENUS_PROMPT` pasa a pedir las instrucciones de elaboración al guardar una receta.
- **Migración aditiva e idempotente**: actualiza `SKILL_MENUS_PROMPT` conservando el encabezado `# SKILL ACTIVA: MENUS`, con **guarda byte a byte** (no pisa un fragmento editado por el usuario) y sin tocar el resto de ajustes.
- **Tests**: se añade cobertura de que el cuerpo enviado a `POST /api/agent/recipes` incluye `instructions` cuando el modelo lo manda y lo omite cuando no; y de que el esquema de la herramienta y el prompt nombran las instrucciones.

## Decisions

- **D1 — El arreglo es de Valet, en el contrato de la herramienta, no en OxiMeal.** No se toca OxiMeal: su API ya acepta `instructions`.
- **D2 — Passthrough sin transformación.** `parse_recipe` sigue enviando el objeto `recipe` tal cual (ya lo hacía); el cambio es **documental** (esquema + prompt) más tests, sin reescribir la lógica de envío.
- **D3 — `update_recipe` queda fuera de alcance.** La Agent API de OxiMeal **no expone `PUT/PATCH`** de recetas, así que no se puede completar las 14 recetas ya creadas desde Valet. Se resuelve por separado (recrearlas, o añadir `PUT` a OxiMeal en un change futuro), no en este.
- **D4 — La migración de prompt usa guarda byte a byte.** Se reconocen los textos conocidos del fragmento anterior y solo se sustituyen esos; cualquier fragmento editado por el usuario se respeta. Precedente: migración `20261104000001` (`widget-options-contract`).
- **D5 — Sin backfill.** Las recetas existentes sin instrucciones no se migran.

## Impact

- **Specs**: ADDED en `oximeal-service` (transmisión de campos opcionales), `tools/registry` (esquema y validación de `create_recipe`) y `orchestrator/skill-router` (fragmento de la skill `menus`); ADDED en `oximeal-api` (migración del prompt).
- **Código backend**: `src/tools/oximeal.rs`, `src/db/repos/settings.rs` (si hiciera falta), `src/orchestrator/skills.rs` (texto por defecto del prompt) y **una migración nueva** en `migrations/`.
- **Base de datos**: una migración aditiva (actualización del valor de `SKILL_MENUS_PROMPT`). Sin cambios de esquema.
- **Frontend**: sin cambios.
- **Dependencias**: ninguna nueva.
- **Fuera de alcance**: cambios en OxiMeal, operación `update_recipe`, backfill de recetas existentes, despliegue y `docker-compose*.yml`.
