# PLAN-028 — El agente entiende la búsqueda de OxiMeal: búsqueda por palabras y aviso accionable

Issue: [#361](https://github.com/atareao/valet-ai/issues/361)
Change OpenSpec: `oximeal-search-semantics`

## 1. Contexto

El change `oximeal-contract-hardening` (PLAN-027) arregló el contrato de `tags` y el mensaje de
error, pero planificar el menú seguía fracasando. Causa: la búsqueda de OxiMeal coincide con una
**subcadena contigua** (FTS5 `trigram`), no con palabras sueltas, y el agente busca con listas de
palabras → 0 resultados → se niega a planificar. Medido en `co1` (2026-11-12): 14 `search_recipes`
en un turno, todas multi-palabra, todas a 0.

## 2. Alcance

- **Búsqueda por palabras en Valet**: `search_recipes` parte la consulta en términos, hace una
  petición por término y combina (intersección primero; si está vacía, por número de coincidencias),
  acotado a 10.
- **Esquema de la herramienta**: la descripción de `query` documenta la semántica.
- **Aviso accionable** cuando no hay coincidencias.
- **Migración** que actualiza `SKILL_MENUS_PROMPT` con la guía (guarda byte a byte).
- OxiMeal **no se toca**.

## 3. Tareas

- [x] 3.1 Change `oximeal-search-semantics` (proposal, deltas, design, tasks) validado en verde.
- [x] 3.2 TDD RED → GREEN → REFACTOR.
- [x] 3.3 Auditoría `@rust-reviewer` sin bloqueantes.
- [x] 3.4 PR a `development` y release a `main`.
- [x] 3.5 Despliegue en `co1` y E2E: planificar el menú con una búsqueda de varias palabras.
- [x] 3.6 `openspec archive oximeal-search-semantics`, índice de `plans/`, issue #361 cerrado.

## 4. Fuera de alcance

- Cambiar OxiMeal.
- Limpiar los duplicados existentes del catálogo de OxiMeal (Merluza/Crema/Lentejas ×2).
