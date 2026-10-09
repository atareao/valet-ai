# Change: Pestaña «Memoria» con sub-pestañas «Episódica» y «Persistente»

## Why

El diálogo de ajustes tiene hoy **dos** pestañas superiores de memoria: «Memoria» (los cuatro
mandos episódicos) y «Memoria persistente» (el editor del estado persistente). Son la misma familia
conceptual repartida en dos entradas del menú, y eso obliga a siete pestañas superiores. Agruparlas
bajo una sola pestaña «Memoria» con sub-pestañas deja seis entradas superiores y ordena el diálogo,
replicando el patrón que ya usan «Prompts» y «Generación».

## What Changes

- **Pestaña «Memoria» con sub-pestañas.** La pestaña superior «Memoria» pasa a contener un `Tabs`
  anidado con dos sub-pestañas —**Episódica** y **Persistente**— envuelto en una región etiquetada (`<section aria-label="Tipo de memoria">`).
  - La sub-pestaña **Episódica** conserva los cuatro mandos actuales (`MEMORY_HALF_LIFE_DAYS`,
    `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS`, `MEMORY_KNN_CANDIDATES`) en el formulario
    compartido.
  - La sub-pestaña **Persistente** aloja el componente `PersistentMemoryPanel`.
  - Ninguna de las dos sub-pestañas usa `forceRender`: antd las monta de forma perezosa, de modo
    que el `GET` del estado persistente solo se dispara al abrir «Persistente».
- **Desaparece la pestaña superior «Memoria persistente».** Su contenido se accede ahora dentro de
  «Memoria» → «Persistente».

## Impact

- Afecta: `frontend/src/components/SettingsDialog.tsx`, `frontend/src/test/SettingsDialog.test.tsx`.
- Specs: `openspec/specs/frontend/spec.md` y `openspec/specs/persistent-memory-ui/spec.md`.
- No cambia contratos de datos ni API: se reutilizan `GET/PUT /settings`, las mismas claves y los
  mismos endpoints del estado persistente. No hay migraciones.
- No toca backend ni `docker-compose.prod.yml`.

### Fuera de alcance

- Renombrar la sub-pestaña «Fichas» de «Generación» (sigue siendo `GENERATION_MEMORY`).
- Cambiar el comportamiento interno del panel de memoria persistente (validación, guardado, 409).
- Cambiar el ancho del `Modal` ni el orden del resto de pestañas superiores.
- Armonizar la accesibilidad de la sub-pestaña de «Generación»: mantiene un `aria-label` sobre
  el `Tabs` que antd no propaga al `role="tablist"` (cosmético). Queda como deuda documentada; este
  cambio solo resuelve «Memoria» con una región etiquetada.
