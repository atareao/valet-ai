# Design

## Context

- Prueba real (contenedor `valet_valet_1`, `deepseek/deepseek-v4.1-flash`) con dos prompts: el modelo
  invocó `render_widget` en ambos y el evento `widget` llegó con `id` no vacío y `done`.
- `data` recibido para `Checklist`: `{ sections: [{ name, items: [{ text }] }] }` → `items` no es array
  → widget vacío.
- `data` recibido para `QuickForm`: `{ title, description, fields: [{ name, label, type, options }],
  submit_label }` con `type: "number"` → casi correcto salvo el tipo numérico.
- La tool declara `data` como objeto genérico (`src/tools/widget.rs:59-62`), sin esquema por widget.

## Goals / Non-Goals

- **Goal:** que el modelo conozca la forma exacta de `data` y que el render tolere pequeñas variantes.
- **Non-Goal:** cambiar el `system_prompt` (decisión del usuario: queda al criterio del LLM), añadir
  widgets nuevos, ni reconstruir widgets al recargar.

## Decisions

- **El esquema va en la tool, no en el prompt.** `parameters()` es lo que el modelo lee para invocar;
  documentar ahí `data` (`oneOf`/descripción con ejemplo) es la corrección de raíz y no depende de la
  configuración del usuario.
- **Ampliar tipos de campo a `number` y `textarea`.** Son los que el modelo intentó usar; el resto se
  mantiene (`text`, `select`, `checkbox`, `slider`) y un tipo desconocido sigue degradando a texto.
- **Tolerancia mínima y general, no específica de un modelo.** `title ?? description` y
  `label ?? text`/`id` sintetizado cubren variantes razonables sin acoplarse a una salida concreta. No
  se aplana `sections` (habría que rehacerlo para cada forma que invente cada modelo).

## Contracts

### `data` documentado en la definición de la tool

Se mantiene `"data": { "type": "object" }` (compatible con todos los proveedores; no se usa `oneOf`,
que algunos validadores de function-calling rechazan) y se añade una `description` enriquecida que
incluye, en texto, la forma de cada widget y un ejemplo:

```jsonc
"data": {
  "type": "object",
  "description": "Datos del widget según widget_name. QuickForm: {\"title\": str, \"fields\": [{\"name\": str, \"label\": str, \"type\": text|textarea|number|select|checkbox|slider, \"options\": [str], \"min\": num, \"max\": num}], \"submit_label\": str}. Checklist: {\"title\": str, \"items\": [{\"id\": str, \"label\": str}]}."
}
```

Tipos de campo: `text | textarea | number | select | checkbox | slider`.

El test comprueba que la `description` de `data` **menciona** las claves y los tipos (es lo que fija el
requisito: «documenta» / «menciona»).

### Tipos TypeScript (`widgets/types.ts`)

`QuickFormField.type` pasa a
`"text" | "textarea" | "number" | "select" | "checkbox" | "slider"`.

## Risks / Trade-offs

- [El modelo sigue desviándose] → La lectura tolerante evita el peor caso (widget vacío); el esquema
  documentado reduce la probabilidad.
- [Documentar el esquema infla el prompt de la tool] → Es texto corto y solo afecta a la definición de
  esta tool.

## Migration Plan

Sin migración. Solo backend (definición de la tool) y frontend (lectura). `docker-compose.prod.yml` no se toca.
