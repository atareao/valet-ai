# Tasks: El prompt del consolidador en la pestaña de Prompts

## Bloque 1 — Tipo y formulario

- [x] 1.1 RED/GREEN: añadir `consolidator_prompt` al tipo de settings (`types/index.ts`) y a
  `SettingsFormValues` de `SettingsDialog`.
- [x] 1.2 RED/GREEN: cargar el valor vigente en el formulario (`setFieldsValue`) y enviarlo en
  `handleSettingsSubmit` sin perder el resto de claves.

## Bloque 2 — Sub-pestaña y aviso

- [x] 2.1 RED/GREEN: sub-pestaña «Consolidator» en la pestaña «Prompts», con área de texto y
  botón de guardado, coherente con *System* / *Archivist* / *Collapse*.
- [x] 2.2 RED/GREEN: aviso no bloqueante al guardar si faltan `{{ ESTADO_ACTUAL }}` o
  `{{ BLOQUE_DE_MENSAJES }}`, nombrando los ausentes; el guardado procede.

## Bloque 3 — Verificación

- [x] 3.1 `npx tsc --noEmit` en verde.
- [x] 3.2 `npx vitest run` en verde, sin regresiones.
- [x] 3.3 `npm run lint:ci` limpio.
- [x] 3.4 `cargo test` sin regresiones (no hay cambios en Rust).
- [x] 3.5 `openspec validate consolidator-prompt-ui --strict` válido.
