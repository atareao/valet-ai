# Design

## Context

Auditoría de cobertura (2026-10-04): `tasks` está cubierta parcialmente por el requisito de permisos de
`tools/registry`; `reminders` y `notes` solo se mencionan; `get_current_time` y `get_current_location`
no tienen spec viva (solo un delta archivado). Las cinco tools corren en el `ToolRegistry` del
orquestador; ninguna expone ruta REST propia.

## Goals / Non-Goals

- **Goal:** fijar por escrito el contrato observable actual de las 5 tools.
- **Non-Goal:** cambiar su comportamiento, ni especificar sus repos/tablas (queda como follow-up).

## Decisions

- **Phase 0, caracterización as-is.** Se documenta lo que el código hace hoy, con sus tests como
  fuente de verdad. Si algo parece discutible (p. ej. que `notes.delete_note` no pida aprobación
  mientras `tasks.delete_task` sí), se documenta tal cual y se anota; cambiarlo sería otro change.
- **Fallback de timezone: Europe/Madrid.** Se corrige el UTC del delta archivado
  `2026-09-27-time-location-tools`, que ya no refleja el código (`src/tools/current_time.rs:40`).
- **Permisos referenciados, no duplicados.** El requisito de permisos de herramientas ya vive en
  `tools/registry`; aquí se documenta el permiso de cada operación de forma local para que cada
  capacidad sea autosuficiente.

## Risks / Trade-offs

- [Riesgo de congelar un comportamiento que en realidad es un bug] → Se anota explícitamente lo
  dudoso; caracterizar no es aprobar.

## Migration Plan

Sin migración. Solo specs.
