# PLAN — Valet (Asistente Inteligente)

**Project Manager:** IA + Lorenzo Carbonell
**Start Date:** 2026-09-01
**Current Phase:** Features Core (Agenda completada)
**Status:** 🟢 Active
**Last Updated:** 2026-09-25

---

## Executive Summary

Valet es un asistente inteligente conversacional con stack **Rust (Axum) + React + SQLite (SQLx)**. Procesa mensajes con streaming vía SSE, ejecuta herramientas (tools) registradas, e inyecta contexto de perfil automáticamente. Usa OpenSpec (SDD) + TDD como metodología de desarrollo.

**Arquitectura:** Monolito backend Rust con frontend React embebido. Despliegue con Podman/Docker. Base de datos SQLite por perfil. Las tools se registran en un `ToolRegistry` y el orquestador (`agent.rs`) las invoca según la respuesta del LLM.

---

## Project Objectives (SMART)

1. **Asistente conversacional funcional**
   - Procesar mensajes con streaming SSE, mantener historial por sesión, ejecutar tools del ecosistema.
   - ✅ Completado: chat básico, streaming, tool calling, inyección de profile_id.

2. **Knowledge Base (RAG + Web Search)**
   - Indexar documentos locales y buscar en web. Almacenar embeddings en SQLite con FTS5.
   - ✅ Completado: ingest, búsqueda semántica, web search, scraping.

3. **Contactos**
   - CRUD de contactos con búsqueda y duplicados.
   - ✅ Completado: create, read, update, delete, search.

4. **Agenda/Calendario (GTD)**
   - Eventos con categorías, recurrencias, día completo, recordatorios y frontend visual.
   - ✅ Completado: DB layer, tool layer, frontend (CalendarView/EventModal/EventDetail).

5. **Recordatorios**
   - Recordatorios simples con listar, crear, completar, descartar.
   - ✅ Completado (básico, sin recurrencias ni notificaciones push).

---

## Scope

### In Scope (Completado)

- [x] Chat conversacional con streaming SSE
- [x] Inyección de profile_id en tools (sin exponerlo al LLM)
- [x] Knowledge: ingest, search, web search, scrap
- [x] Contactos: CRUD + search
- [x] Agenda: eventos, categorías, recurrencias (RRULE), día completo, recordatorios
- [x] Frontend calendario: CalendarView, EventModal, EventDetail
- [x] Recordatorios básicos

### Out of Scope (Ahora)

- ❌ GTD Tasks (proyectos, contextos, próximas acciones)
- ❌ Sistema de plugins (tools externas)
- ❌ Unified Search (búsqueda en todas las fuentes)
- ❌ Notificaciones push
- ❌ Multi-idioma
- ❌ Autenticación OAuth

---

## Arquitectura Actual

```
┌──────────────────────────────────────────────────┐
│                   Frontend React                   │
│  AppLayout → Modal → CalendarView                 │
│  API Client → fetch → /api/*                      │
│  Hooks: useEvents, useContactos, etc.              │
└──────────────────┬───────────────────────────────┘
                   │ HTTP/SSE
┌──────────────────▼───────────────────────────────┐
│               Backend Rust (Axum)                 │
│                                                   │
│  POST /api/chat           → agent::process_msg    │
│  POST /api/chat/stream    → agent::process_stream │
│  GET  /api/events         → list_events           │
│  POST /api/events         → create_event          │
│  PUT  /api/events/:id     → update_event          │
│  DELETE /api/events/:id   → delete_event          │
│                                                   │
│  ToolRegistry:                                    │
│    ├─ CalendarTool  (agenda, eventos, categorías) │
│    ├─ RemindersTool (recordatorios)               │
│    ├─ ContactsTool  (contactos)                   │
│    ├─ KnowledgeTool (RAG + web search)            │
│    └─ WebSearchTool (búsqueda internet)           │
│                                                   │
│  Orquestador (agent.rs):                          │
│    LLM call → parse tool_call → inject            │
│    profile_id → registry.execute() →              │
│    append result → loop hasta finish              │
└──────────────────┬───────────────────────────────┘
                   │ SQLx
┌──────────────────▼───────────────────────────────┐
│              SQLite (por perfil)                   │
│  events, contacts, knowledge_chunks,              │
│  conversations, reminders, profiles               │
│  FTS5 sobre knowledge_chunks + contacts           │
└──────────────────────────────────────────────────┘
```

---

## Timeline & Milestones

| Fase | Fecha | Estado |
|:---|---:|:---:|
| **F0**: Chat + streaming + arquitectura base | 2026-09-01 | ✅ |
| **F1**: Knowledge Base (RAG + Web Search) | 2026-09-10 | ✅ |
| **F2**: Contactos | 2026-09-15 | ✅ |
| **F3**: Agenda/Calendario completa | 2026-09-25 | ✅ |
| **F4**: Recordatorios | 2026-09-25 | ✅ (básico) |
| **F5**: ? (próxima feature) | TBD | ⏸️ |

---

## Work Breakdown

### F0 — Fundación
- [x] Proyecto Rust (Axum) + React (Vite + Ant Design)
- [x] Chat con streaming SSE
- [x] ToolRegistry + orquestador agent.rs
- [x] Inyección de profile_id
- [x] Docker/Podman compose

### F1 — Knowledge
- [x] Ingest de documentos (PDF, texto, web)
- [x] Embeddings + búsqueda semántica (FTS5)
- [x] Web search integration
- [x] Web scraping

### F2 — Contactos
- [x] DB schema + migración
- [x] CRUD + búsqueda
- [x] Tool layer + tests

### F3 — Agenda (completada 2026-09-25)
- [x] **T0**: Fix profile_id injection
- [x] **T1**: Registrar CalendarTool + RemindersTool
- [x] **T2**: DB Layer (events extendido con category, all_day, rrule, reminder)
- [x] **T3**: Tool Layer (delete, list_by_category, expand_recurring)
- [x] **T4**: Frontend (CalendarView, EventModal, EventDetail)
- [x] **T5**: Verificación (412 Rust tests, 16 frontend tests, clippy, fmt, tsc)

### F4 — Recordatorios
- [x] Recordatorios básicos (create, list, complete, dismiss)
- [ ] Mejoras futuras: recurrencias, categorías, notificaciones

---

## Stack Técnico

| Capa | Tecnología |
|:---|---:|
| Backend | Rust + Axum + SQLx + Tokio |
| Frontend | React 18 + TypeScript + Vite + Ant Design |
| Base de datos | SQLite (por perfil) + FTS5 |
| Contenedores | Podman + Docker Compose |
| Metodología | OpenSpec (SDD) + TDD (Red-Green-Refactor) |

### Comandos clave

```bash
cargo test# tests Rust (412)
cargo clippy -- -D warnings# lint Rust
cargo fmt --check# formateo Rust
npx tsc --noEmit# type check React
npx vitest run# tests React (16)
just dev# levantar con Podman
just dev-docker# levantar con Docker
```

---

## Próximas Features (Opciones)

| Opción| Descripción |Prioridad |Esfuerzo estimado |
|:---|---:|---:|---:|
|🗂️ **GTD Tasks**|Sistema completo de tareas con proyectos, contextos, próximas accioness, @etiquetas, revisión semanal|Alta| 2-3 semanas||
|🌐**UnifiedSearch** |Búsqueda unificada en knowledge + contactoss + agenda + tasks|Media |1 semana||
|📱 **Mejora Reminders** |Recordatorios con recurrencias, categorías, notifcaciones push |Bja |1 semana||
|🤖**Plugin Systm**|Permitir tools externas sin modificar el código base |Baja| 3-4 semanas||


---

## Notas & Historial de Cambios

### Change Proposals Archivados

| Feature | Archivo | Fecha |
|:---|---:|---:||
| `complete-agenda` | `openspec/changes/2026-95-25-complete-agenda` |2026-09-25 |

### Especificaciones en openspec/specs/

- `db/schema/specc.md` — Esquema de BBDD (events, conttacts, knowledge, etc.)
- `frontend/agenda/spec.md` — Componentes e interfaz de usuario de agenda
- `orchestrator/profile-injection/spec.md` — Inyección de profile_id en el orquestador
- `tools/agenda/spec.md` — CalendarTool y RemindersTool