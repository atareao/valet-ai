# Change: oximeal-search-semantics

## Why

Tras arreglar el contrato de `tags` (issue #358, v0.36.1), `search_recipes` ya no falla, pero
**planificar el menú semanal sigue fracasando**: el agente busca con **listas de palabras**
(«tomate aguacate mozzarella», «salmón espárragos») y OxiMeal, cuya búsqueda coincide con una
**subcadena contigua** del nombre o las instrucciones (FTS5 `trigram`, frase entre comillas),
devuelve **0**. Con 0 resultados el agente se niega a planificar («*No pienso meter ids a ciegas
otra vez*») en vez de llamar a `plan_week`. Medido en `co1` el 2026-11-12: **14 `search_recipes`
en un turno**, todas multi-palabra, todas a 0. `recent_recipes` no ayuda: lista recetas
**planificadas**, no **creadas**. Issue [#361](https://github.com/atareao/valet-ai/issues/361).

## What Changes

- **Búsqueda por palabras en el servicio.** `search_recipes` interpreta `query` como palabras: un
  término → una petición; varios → una petición por término (con el mismo `category`) y
  combinación en Valet —primero la intersección por `id`; si está vacía, por número de
  coincidencias— acotada al máximo de la operación.
- **Esquema de la herramienta.** La descripción del parámetro `query` documenta que OxiMeal busca
  por subcadena contigua y que Valet combina varios términos.
- **Aviso accionable.** Cuando `search_recipes` no encuentra nada, la herramienta devuelve un
  mensaje que explica la semántica y sugiere un término corto y distintivo.
- **Prompt de la skill `menus`.** Una migración nueva actualiza `SKILL_MENUS_PROMPT` con la guía de
  búsqueda (guarda byte a byte).
- **Sin cambios** en OxiMeal, en los endpoints HTTP, en los ajustes ni en el frontend.

## Capabilities

### New Capabilities

_(ninguna)_

### Modified Capabilities

- `oximeal-service`: la búsqueda de recetas combina los términos de la consulta.
- `tools/registry`: el esquema de `search_recipes` documenta la semántica de `query` y la
  herramienta avisa cuando no hay coincidencias.
- `oximeal-api`: una migración actualiza `SKILL_MENUS_PROMPT` con la guía de búsqueda.

## Impact

- Código: `src/services/oximeal.rs` (`search_recipes`), `src/tools/oximeal.rs` (esquema y aviso),
  una migración nueva y sus tests.
- Documentación: `docs/oximeal-integration.md`.
- Sin migraciones de datos, sin ajustes nuevos, sin frontend, sin changes de despliegue.
- Fuera de alcance: cambiar OxiMeal y limpiar los duplicados existentes del catálogo.
