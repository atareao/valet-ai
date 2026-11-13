# Design: oximeal-contract-hardening

## Decisión 1 — Tolerar las etiquetas objeto y exponer nombres

**Contexto.** OxiMeal v0.10.0 (`a98dbe0`) cambió `RecipeSummary.tags` de `Vec<String>` a
`Vec<RecipeTag>` (`{id, name, slug, source}`). Valet declaraba `Vec<String>`, así que toda búsqueda
fallaba.

**Alternativas descartadas.**
- Decodificar `tags` como `Vec<serde_json::Value>` y dejar el JSON crudo: expondría al modelo ids,
  slugs y `source`, ruido innecesario y superficie inestable.
- Aceptar ambas formas con un `enum` externo (`#[serde(untagged)]`): más código y más pruebas para un
  caso que el servidor ya no emite.

**Decisión.** Un tipo `TagRef { name: String }` con `#[serde(default)]` y campos extra ignorados; la
salida de la herramienta sigue siendo la lista de **nombres**, sin cambiar el JSON que ya ve el modelo.

## Decisión 2 — Preferir `message` sobre `error`

El cuerpo de error de OxiMeal es `{"error":"validation","message":"datos inválidos: …"}`. `server_message`
leía `error` primero y devolvía el código máquina. Se reordena a `message → error → detail → title`.
El fallback al cuerpo recortado se mantiene para respuestas no JSON.

## Decisión 3 — Documentar la procedencia del `recipe_id` en vez de pre-validar

**Alternativas descartadas.**
- Pre-validar cada `recipe_id` con una llamada a `search_recipes` antes de `plan_week`: añade latencia y
  una carrera (la receta puede cambiar entre ambas), y convierte un `400` claro en lógica duplicada.
- Rechazar `recipe_id` que no sea UUID: frágil (el formato del id es del servidor) y bloquea ids válidos
  si OxiMeal cambia su generación.

**Decisión.** Documentar en el esquema que el `recipe_id` sale de `search_recipes`/`create_recipe`, y
dejar que el `400` real —ahora con el mensaje humano, gracias a la Decisión 2— corrija al modelo dentro
del turno. El defecto 1 (búsqueda rota) era lo que dejaba al modelo sin ids reales; arreglarlo es la
mitigación principal del defecto 2.

## Consecuencias

- Sin migraciones, sin ajustes nuevos, sin cambios de frontend.
- OxiMeal no se toca.
- El modelo puede seguir inventando ids: esta spec fija el contrato de Valet, no el comportamiento del
  modelo; la mitigación es la documentación y el error accionable.
