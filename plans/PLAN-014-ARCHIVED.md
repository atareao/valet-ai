# PLAN-014 — La proyección de widgets reconoce los formatos reales del modelo

- **Estado:** ✅ completado el 2026-11-03 (release v0.27.0 desplegado y verificado en producción).
- **Fecha:** 2026-11-02
- **Origen:** el cierre medido de `PLAN-013` / change `widget-projection`: la verificación desplegada (10.4) midió un falso negativo.
- **Change de OpenSpec:** `openspec/changes/widget-projection-formats/`

## 1. Objetivo

Conseguir que la proyección determinista de widgets dispare sobre los formatos que el modelo emite de
verdad, no solo sobre las formas ideales que el detector asumió al diseñarse. El change anterior
(`widget-projection`, v0.26.0) dejó el mecanismo en pie y verificado por tests, pero la primera medición
sobre un despliegue real demostró que el detector no reconocía el texto que el modelo produce
habitualmente. Este plan cierra esa brecha sin cambiar el esquema, el payload del widget ni el frontend.

## 2. Evidencia medida

La verificación desplegada del 2026-11-02 (tarea 10.4 del change `widget-projection`) envió un turno real
sobre la imagen v0.26.0, con una copia consistente de la BD de producción en un contenedor aislado
(`AUTH_ENABLED=false`, puerto 3010). La respuesta del modelo fue una lista numerada de opciones:

```
1. **Cine: una película en el centro**
2. **Cena: un restaurante nuevo**
3. **Tarde de gimnasio: entrenamiento libre**

¿Cuál prefiere?
```

Resultado: **falso negativo**. No se emitió ningún evento `widget` y `messages.widgets` quedó `null`.

La causa raíz exacta está en `question_options_block` (`src/orchestrator/widget_projection.rs`): la
función exige la interrogativa de cierre en la línea **inmediatamente** siguiente al último ítem de la
lista. En el texto real hay una **línea en blanco** entre el último ítem y «¿Cuál prefiere?», de modo que
la comprobación recae sobre la línea vacía y la función devuelve `None`. A esto se suman dos huecos:
el texto de las opciones conserva los marcadores de énfasis Markdown (`**…**`) y las opciones presentadas
como tabla Markdown no las reconoce ningún detector.

## 3. Alcance

Tres cambios, todos en el detector:

1. **Tolerancia a líneas en blanco** entre la lista y la interrogativa de cierre, tomando como
   interrogativa la primera línea no vacía posterior. El `span` sigue delimitando solo el bloque de la
   lista, sin incluir las líneas en blanco ni la interrogativa.
2. **Ignorar el énfasis Markdown** (`**…**`, `__…__`, `*…*`, `_…_`, `` `…` ``) al extraer etiquetas,
   opciones e ítems, conservando el texto interior y sin alterar el `span`, que se calcula sobre el texto
   original.
3. **Detector de tabla de opciones**, con precedencia tras el detector de tabla de checklist, que
   reconoce tablas con etiquetas consecutivas (`A`, `B`, `C` / `1`, `2`, `3`) o con cabecera que nombre
   opciones (`Opción`/`Alternativa`/`Option`).

## 4. Mapa de tareas

El desglose TDD (RED / GREEN / REFACTOR / cierre) vive en
`openspec/changes/widget-projection-formats/tasks.md`. El plan no duplica la checklist.

## 5. Verificación

Repetir la verificación desplegada 10.4/10.5 —trasladada desde el change `widget-projection`— sobre un
release nuevo: comprobar en la BD que `messages.widgets` deja de estar vacío con un turno que ahora sí
proyecte, que el contenido persistido conserva la lista íntegra, que el widget lleva un `span` que
delimita el bloque y que la UI muestra el widget en el sitio de la lista.

## 6. Relevo

El estado del trabajo vive en los artefactos de OpenSpec (`proposal.md`, `design.md`, `tasks.md`, el
delta de spec) y en el historial de git, no en la conversación. El change ya está archivado y este plan
quedó como `plans/PLAN-014-ARCHIVED.md`.

## 7. Resultado

- Change implementado, auditado (`@rust-reviewer`: APTO) y **desplegado como v0.27.0** (`ghcr.io/atareao/valet-ai:latest`, PR #291 a desarrollo y #292 a main).
- Verificación end-to-end (10.4) el 2026-11-03 sobre la imagen v0.27.0 con copia de la BD de producción: un turno real cuya respuesta presenta una lista de opciones (con **negrita** en cada ítem y cerrada por una interrogativa) acabó con **1 widget proyectado** con `span` que delimita exactamente el bloque de la lista, contenido persistido íntegro y opciones sin los marcadores `**`.
- Nota de honestidad: el disparo depende de que el modelo *presente* opciones (comportamiento no determinista); con un prompt que forzó una lista, la proyección disparó y persistió correctamente.
- Se **archiva** el change y se sincroniza `development` con `main`.
