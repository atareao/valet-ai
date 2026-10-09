# Tasks: settings-memory-tabs

## Bloque 1 — RED (frontend)
- [x] 1.1 Test: la pestaña «Memoria» muestra un `Tabs` anidado con las sub-pestañas Episódica y Persistente (falla hoy: «Memoria persistente» es pestaña superior).
- [x] 1.2 Test: los cuatro mandos `MEMORY_*` se ven en la sub-pestaña «Episódica» y se guardan.
- [x] 1.3 Test: al seleccionar «Persistente» se monta el panel; sin seleccionarla, NO está montado.
- [x] 1.4 Test: las pestañas superiores son exactamente seis y NO existe «Memoria persistente» como pestaña superior.
- [x] 1.5 Verificar por CLI: `npx vitest run` → los nuevos en rojo y los legacy en verde (176).

## Bloque 2 — GREEN (frontend)
- [x] 2.1 En «Memoria», montar un `Tabs` anidado (envuelto en `<section aria-label="Tipo de memoria">`) con «Episódica» y «Persistente», sin `forceRender` (montaje perezoso).
- [x] 2.2 Mover el formulario de los cuatro mandos a la sub-pestaña «Episódica» y `PersistentMemoryPanel` a «Persistente».
- [x] 2.3 Eliminar la pestaña superior «Memoria persistente».
- [x] 2.4 Verificar por CLI: `npx vitest run`, `npx tsc --noEmit`, `npm run lint:ci`.

## Bloque 3 — REFACTOR y revisión
- [x] 3.1 Revisión con `react-reviewer` (sin blockers).
- [x] 3.2 Verificación final por CLI y PR a `development`.
