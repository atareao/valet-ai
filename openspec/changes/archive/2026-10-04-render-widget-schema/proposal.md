# Proposal

## Why

En la prueba real contra el modelo (contenedor desplegado), un prompt como «hazme un checklist para
preparar una mudanza» hace que el modelo invoque `render_widget` por sí solo y el evento SSE llega con
`id`. Pero la forma de `data` **es inventada**, porque la tool la declara como `"data": {"type":"object"}`
sin documentar el esquema:

- `Checklist`: el modelo envió `{ sections: [{ name, items: [{ text }] }] }` en vez de
  `{ title, items: [{ id, label }] }` → el widget se renderiza **vacío**.
- `QuickForm`: envió `title`, `fields[{name,label,type}]` y `submit_label` (casi correcto), pero con
  `type: "number"`, que hoy degrada a un campo de texto.

Es un fallo de contrato, no de la tubería: el transporte y el render degradan sin romper, pero el widget
resulta inútil. Se corrige documentando el esquema en la propia tool y ampliando/endureciendo la lectura
en el frontend.

## What Changes

- **`tools/registry` (ADDED)**: la definición de `render_widget` documenta en `parameters()` el esquema
  de `data` de cada widget (`QuickForm` y `Checklist`), con los tipos de campo admitidos y un ejemplo.
- **`frontend` (MODIFIED)**: `QuickForm` admite los tipos de campo `number` y `textarea`.
- **`frontend` (ADDED)**: tolerancia de lectura: `QuickForm` usa `description` como título si falta
  `title`; `Checklist` acepta ítems con `label` o `text` y sintetiza `id` cuando falta.

**Fuera de alcance (decisión del usuario):** no se toca el `system_prompt` para guiar el uso de widgets;
queda al criterio del LLM.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `tools/registry` (se añade un requisito).
- `frontend` (se modifica un requisito y se añade otro).

## Impact

- **Backend**: solo `src/tools/widget.rs` (`parameters()`/`description()`). Sin cambios de lógica ni de BD.
- **Frontend**: `frontend/src/components/widgets/QuickFormWidget.tsx` y `ChecklistWidget.tsx` (y
  `types.ts` para el tipo de campo). Sin dependencias nuevas.
- **No** se toca `docker-compose.prod.yml` ni el `system_prompt`.
