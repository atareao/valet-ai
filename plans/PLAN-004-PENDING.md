# Valet v2 — Roadmap

> Nuevas dimensiones para el Life Operating System: gestión de contenido editorial,
> bitácora personal y bitácora de proyectos.

---

## 1. Gestión de atareao.es

### Visión

Valet gestiona todo el ciclo de vida del contenido de **atareao.es**: podcast, artículos del blog, tutoriales, vídeos de YouTube. Desde la idea inicial hasta la publicación, todo orquestado desde una conversación con Valet.

### Dimensiones de contenido

| Tipo | Descripción | Plataforma destino |
|------|-------------|-------------------|
| **Podcast** | Episodios semanales "atareao con Linux" | Spotify, YouTube, iVoox, web |
| **Artículos** | Posts técnicos del blog | atareao.es (WordPress) |
| **Tutoriales** | Guías paso a paso con ejemplos | atareao.es, YouTube |
| **Vídeos** | Contenido audiovisual | YouTube |

### Flujo completo (de idea a publicación)

```
1. 💡 IDEA
   │  "Valet, tengo una idea para un artículo sobre Podman"
   │  Valet guarda la idea y sugiere formato (podcast / artículo / vídeo)
   │
   ▼
2. 🔬 INVESTIGACIÓN
   │  "Valet, investiga sobre Podman 5.0"
   │  Valet busca en web (SearXNG), docs oficiales, repos GitHub
   │  Genera un brief con fuentes, novedades, ejemplos clave
   │
   ▼
3. 📝 GUION / ESCRITURA
   │  "Valet, escribe el guion del podcast"
   │  Valet genera guion estructurado siguiendo la house style
   │  Incluye: intro, desarrollo, ejemplos prácticos, despedida
   │  Para artículos: genera borrador con SEO titles y meta description
   │
   ▼
4. 👁️ REVISIÓN
   │  "Valet, revisa el artículo"
   │  Valet aplica: blog-avoid-ai, blog-house-style, blog-edit
   │  Sugiere mejoras, detecta AI-isms, verifica código de ejemplo
   │  Ciclo de revisión hasta aprobación
   │
   ▼
5. 🎬 PRODUCCIÓN
   │  "Valet, prepara los metadatos del episodio"
   │  Valet genera:
   │    • Título y descripción para YouTube (SEO)
   │    • Título y descripción para Spotify
   │    • Miniaturas (prompt para DALL-E / Midjourney)
   │    • Capítulos con timestamps
   │    • Mostrador de código para la demo
   │
   ▼
6. 🚀 PUBLICACIÓN
   │  "Valet, publica el artículo"
   │  Valet:
   │    • Sube a WordPress vía REST API
   │    • Programa publicación en fecha/hora
   │    • Actualiza redes sociales
   │    • Notifica en el canal de Telegram/Discord
   │
   ▼
7. 📊 SEGUIMIENTO
   │  "Valet, qué tal funcionó el último artículo"
   │  Valet consulta analytics (si están disponibles)
   │  Muestra visitas, reproducciones, engagement
```

### Tools propuestas

| Tool | Operaciones |
|------|-------------|
| `content_ideas` | `add_idea`, `list_ideas`, `suggest_format`, `brainstorm` |
| `content_research` | `research_topic`, `gather_sources`, `generate_brief` |
| `content_writing` | `write_script` (podcast), `write_article` (blog), `write_tutorial`, `apply_house_style`, `remove_ai_isms` |
| `content_review` | `review_draft`, `check_code_examples`, `suggest_improvements` |
| `content_metadata` | `generate_titles`, `generate_description_youtube`, `generate_description_spotify`, `generate_chapters`, `generate_thumbnail_prompt` |
| `content_publish` | `publish_wordpress`, `schedule_post`, `notify_channels` |
| `content_analytics` | `get_stats`, `top_content`, `engagement_report` |

### Nuevos workers

| Worker | Descripción |
|--------|-------------|
| **ContentScheduler** | Revisa el calendario editorial cada mañana, recuerda fechas de publicación |
| **AnalyticsCollector** | Semanal: recopila métricas de WordPress, YouTube, Spotify |

---

## 2. Bitácora

### Visión

Una **bitácora personal** (journal/diario) que Valet mantiene de forma proactiva. No es solo "escribir notas" — es un sistema que captura el día a día, extrae aprendizajes, y construye una narrativa personal.

### Diferencias con notes existente

| Aspecto | Notes (F5b) | Bitácora (v2) |
|---------|-------------|---------------|
| Propósito | Notas rápidas, ideas sueltas | Narrativa personal diaria |
| Estructura | Libre, categorías | Fechas, estados de ánimo, logros |
| Proactividad | Solo creación manual | Valet pregunta "¿Cómo fue tu día?" |
| Integración | Aislada | Se alimenta de eventos, hábitos, clima |
| Reflexión | No | Valet sugiere patrones, celebra rachas |

### Flujo

```
20:00 — Valet pregunta: "¿Cómo fue tu día?"
         │
         ▼
Usuario responde: "Bien, terminé el proyecto X, pero tuve dolor de cabeza"
         │
         ▼
Valet estructura la entrada:
  • Fecha: 2026-09-24
  • Ánimo: 😊 (positivo)
  • Logros: Proyecto X completado
  • Incidencias: Dolor de cabeza
  • Contexto: Clima 22°C, sin eventos destacados
  • Tags: trabajo, salud
         │
         ▼
Valet guarda en `journal_entries` y extrae hechos para memoria
```

### Schema propuesto

```sql
CREATE TABLE journal_entries (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id),
    date TEXT NOT NULL,                  -- ISO date (único por perfil)
    mood TEXT,                           -- "great", "good", "neutral", "bad", "terrible"
    content TEXT NOT NULL,
    achievements TEXT,                   -- JSON array de logros
    struggles TEXT,                      -- JSON array de dificultades
    tags TEXT,                           -- JSON array
    weather_context TEXT,                -- clima del día (JSON)
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(profile_id, date)
);
```

### Tools propuestas

| Tool | Operaciones |
|------|-------------|
| `journal` | `get_entry(date?)`, `write_entry`, `update_entry`, `list_entries(range?)`, `mood_timeline(range?)` |
| `journal_insights` | `weekly_summary`, `mood_patterns`, `achievement_tracker`, `suggest_reflection` |

### Workers

| Worker | Descripción |
|--------|-------------|
| **JournalPrompt** | Cada día a las 20:00 pregunta "¿Cómo fue tu día?" si no hay entrada |
| **JournalDigest** | Domingo: genera resumen semanal de la bitácora |

---

## 3. Bitácora de un Proyecto

### Visión

Un **sistema de seguimiento de proyectos** que va más allá de las tareas. Cada proyecto tiene su propia bitácora: decisiones, aprendizajes, hitos, blockers, y evolución. Ideal para proyectos personales y profesionales.

### Diferencias con tasks existente

| Aspecto | Tasks (F5b) | Project Log (v2) |
|---------|-------------|-------------------|
| Propósito | Tareas atómicas | Narrativa del proyecto |
| Scope | Días/semana | Meses/años |
| Estado | pending/completed | En evolución continua |
| Contexto | Sin contexto | Decisiones, aprendizajes, referencias |
| Relaciones | Aisladas | Enlaza a notas, commits, PRs, docs |

### Estructura de un proyecto

```json
{
  "id": "proj-abc",
  "name": "Valet v2",
  "status": "in_progress",    // "idea", "planning", "active", "paused", "done", "abandoned"
  "vision": "Un asistente que...",
  "start_date": "2026-09-01",
  "target_date": "2026-12-31",
  "tags": ["rust", "ai", "personal"],

  "milestones": [
    { "name": "MVP", "date": "2026-10-01", "completed": true },
    { "name": "Beta", "date": "2026-11-15", "completed": false }
  ],

  "decisions": [
    {
      "date": "2026-09-10",
      "title": "SQLite vs PostgreSQL",
      "decision": "SQLite con sqlite-vec",
      "rationale": "Simplicidad, embeddings vectoriales, sin servidor",
      "alternatives": ["PostgreSQL + pgvector"]
    }
  ],

  "learnings": [
    {
      "date": "2026-09-15",
      "title": "sqlite-vec requiere load_extension",
      "detail": "No funciona con rusqlite bundled...",
      "links": ["https://..."]
    }
  ],

  "links": [
    { "url": "https://github.com/...", "description": "Repo" },
    { "url": "https://...", "description": "Doc de diseño" }
  ]
}
```

### Schema propuesto

```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id),
    name TEXT NOT NULL,
    vision TEXT,
    status TEXT NOT NULL DEFAULT 'idea'
        CHECK(status IN ('idea', 'planning', 'active', 'paused', 'done', 'abandoned')),
    start_date TEXT,
    target_date TEXT,
    tags TEXT,                           -- JSON array
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE project_milestones (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0,
    notes TEXT
);

CREATE TABLE project_decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    date TEXT NOT NULL,
    title TEXT NOT NULL,
    decision TEXT NOT NULL,
    rationale TEXT NOT NULL,
    alternatives TEXT,                    -- JSON array
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE project_learnings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    date TEXT NOT NULL,
    title TEXT NOT NULL,
    detail TEXT NOT NULL,
    links TEXT,                           -- JSON array
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE project_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    date TEXT NOT NULL,
    content TEXT NOT NULL,                -- Entrada de bitácora del proyecto
    entry_type TEXT NOT NULL DEFAULT 'general'
        CHECK(entry_type IN ('general', 'progress', 'blocker', 'decision', 'learning')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Tools propuestas

| Tool | Operaciones |
|------|-------------|
| `projects` | `create_project`, `list_projects`, `get_project`, `update_status`, `add_milestone`, `complete_milestone` |
| `project_log` | `add_entry`, `list_entries`, `add_decision`, `add_learning`, `add_link`, `get_timeline` |
| `project_report` | `weekly_summary`, `progress_report`, `blockers_list`, `decision_log` |

### Workers

| Worker | Descripción |
|--------|-------------|
| **ProjectReminder** | Cada semana pregunta "¿Cómo va el proyecto X?" si no hay entradas recientes |
| **ProjectCheckpoint** | Detecta proyectos inactivos (>14 días sin entradas) y pregunta si continuar o pausar |

---

## Resumen v2

| Feature | Tools | Tablas nuevas | Workers |
|---------|-------|---------------|---------|
| **atareao.es** | 6 tools (ideas, research, writing, review, metadata, publish) | 3-4 tablas (content_pipeline, episodes, posts) | ContentScheduler, AnalyticsCollector |
| **Bitácora** | 2 tools (journal, insights) | 1 tabla (journal_entries) | JournalPrompt, JournalDigest |
| **Project Log** | 3 tools (projects, project_log, project_report) | 5 tablas (projects, milestones, decisions, learnings, entries) | ProjectReminder, ProjectCheckpoint |
| **Total v2** | 11 tools | 9-10 tablas | 6 workers |

