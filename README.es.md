# 🎩 Valet

[English](README.md)

El asistente personal de IA con actitud.

Valet es una IA personal, personalizable y muy enfocada, diseñada para encargarse de tus flujos de trabajo, tareas y consultas del día a día. Está construida para la velocidad y la utilidad: te sirve con precisión y con una buena dosis de humor británico seco y afilado.

Auto-hospedado y local-first: tus datos viven en SQLite en tu propia máquina, y el asistente responde con la educación de un mayordomo competente y la paciencia de ninguno.

## 🗂️ Características

- **🫖 Chat con IA**: Orquestador con ciclo ReAct y memoria por capas (sesión, episódica, perfil)
- **🕰️ Agenda y Tareas**: Gestión de eventos, tareas y recordatorios con scope `shared`/`personal`
- **🗺️ Clima y Geo**: Clima por coordenadas, geocoding (Nominatim), búsqueda de lugares (Overpass OSM)
- **🧐 Búsqueda Unificada**: FTS5 en todas las dimensiones
- **🗝️ Privado**: Datos locales en SQLite, auto-hospedado, con auth PocketID opcional
- **🔔 Proactivo**: Briefing matutino, detección de conflictos, preparación de viajes
- **📱 UI Responsive**: Frontend React + Ant Design que funciona en escritorio y móvil

## 🧠 Memoria Episódica

Valet cuenta con un sistema de memoria episódica de dos capas:

- **Capa A (mensajes en bruto)**: la tabla `messages` con cada interacción usuario↔asistente
- **Capa B (fichas episódicas)**: la tabla `memory` con resúmenes sintéticos generados por un LLM secundario

El worker `EpisodicMemoryWorker` se dispara al insertar un mensaje (con un timer de respaldo de 30 minutos):

1. Acumula mensajes sin indexar hasta ~2000 tokens
2. Añade ±2 mensajes de solapamiento para contexto
3. Envía el bloque a un LLM secundario con prompt de archivista
4. Genera una ficha estructurada (`FECHA`, `TEMAS`, `HECHOS`, `SÍNTESIS`)
5. Almacena la ficha + embedding vectorial en `memory` + `vec_memory`
6. Marca los mensajes como indexados

En el chat, el orquestador usa **RAG**: genera el embedding de la consulta del usuario, busca en `vec_memory` por similitud coseno, e inyecta las fichas más relevantes en el contexto respetando un presupuesto de tokens.

## 🧱 Stack

| Capa | Tecnología |
|------|-----------|
| Backend | Rust + Axum 0.8 + Tokio + sqlx 0.9 |
| Frontend | TypeScript + React 18 + Ant Design 5 + Vite |
| Base de datos | SQLite + FTS5 + sqlite-vec |
| Auth | PocketID (OIDC self-hosted), opcional |
| LLM | OpenRouter (principal) / Ollama (fallback) |
| Contenedores | Docker + Docker Compose (Podman para desarrollo local) |

## 📋 Requisitos

- Rust 1.94+
- Node.js 22+
- SQLite 3.45+ (con FTS5)
- Docker o Podman + Compose (opcional)

## 🚀 Inicio rápido

El punto de entrada recomendado es [`just`](https://github.com/casey/just):

```bash
# 1. Clonar
git clone https://github.com/atareao/valet-ai.git
cd valet-ai

# 2. Configurar
cp .env.example .env
# edita .env y añade al menos OPENROUTER_API_KEY

# 3. Levantar con Podman (reconstruye la imagen)
just dev
# o con Docker
just dev-docker

# Servidor: http://localhost:3000
```

### Ejecución sin contenedores

```bash
# Backend
cp .env.example .env
cargo run

# Frontend (otra terminal)
cd frontend
npm install
npm run dev
```

## ⚙️ Configuración

Las variables de entorno se leen una vez al arrancar (ver `src/config.rs`) con defaults sensatos. Las principales (ver `.env.example`):

| Variable | Descripción | Default |
|----------|-------------|---------|
| `HOST` | Dirección de bind | `0.0.0.0` |
| `PORT` | Puerto HTTP | `3000` |
| `DATABASE_URL` | Ruta a la BD SQLite | `valet.db` |
| `LOG_LEVEL` | Nivel de log | `info` |
| `OPENROUTER_API_KEY` | API key de OpenRouter | — |
| `OPENROUTER_MODEL` | Modelo principal de chat | `anthropic/claude-sonnet-20241022` |
| `OPENROUTER_BASE_URL` | URL base de la API de OpenRouter | `https://openrouter.ai/api/v1` |
| `OLLAMA_BASE_URL` | URL base del fallback Ollama | `http://localhost:11434` |
| `OLLAMA_MODEL` | Modelo de fallback Ollama | `llama3.2:3b` |
| `AUTH_ENABLED` | Habilitar autenticación PocketID | `false` |
| `AUTH_ISSUER_URL` | URL del emisor OIDC (requerida cuando `AUTH_ENABLED=true`) | — |
| `AUTH_CLIENT_ID` | Client ID OIDC (requerido cuando `AUTH_ENABLED=true`) | — |
| `AUTH_CLIENT_SECRET` | Client secret OIDC (requerido cuando `AUTH_ENABLED=true`) | — |
| `AUTH_REDIRECT_URL` | URL de redirección OIDC — Redirect URI del cliente PocketID (`https://TU_DOMINIO/api/auth/callback`); requerida cuando `AUTH_ENABLED=true` | — |
| `AUTH_POST_LOGOUT_REDIRECT_URL` | URI de redirección tras el logout enviada al proveedor (opcional) | — |
| `JWT_SECRET` | Secreto para firmar tokens de sesión (requerido cuando `AUTH_ENABLED=true`) | — |
| `OPENWEATHER_API_KEY` | API key de OpenWeather | — |
| `GOOGLE_PLACES_API_KEY` | API key de Google Places | — |
| `BRAVE_SEARCH_API_KEY` | API key de Brave Search | — |
| `COLLAPSE_THRESHOLD_TOKENS` | Tokens antes del colapso de contexto | `2000` |
| `COLLAPSE_MODEL` | Modelo para el colapso de contexto | `mistralai/mistral-small-24b-instruct-2501` |
| `MEMORY_BATCH_TOKENS` | Tokens acumulados para trigger de ficha episódica | `2000` |
| `MEMORY_INACTIVITY_MINUTES` | Minutos de inactividad para forzar ficha | `30` |
| `MEMORY_OVERLAP` | Mensajes de solapamiento (±) en el bloque | `2` |
| `MEMORY_POLL_INTERVAL_MINUTES` | Intervalo del timer de respaldo | `30` |
| `MEMORY_MODEL` | Modelo para generar fichas episódicas | `mistralai/mistral-small-24b-instruct-2501` |
| `RAG_BUDGET_TOKENS` | Presupuesto de tokens para RAG en el chat | `2000` |

## 🏭 Producción

`docker-compose.prod.yml` separa el stack en un backend Rust (`Dockerfile.backend`, musl, corre como usuario no-root UID 1000), un frontend nginx independiente (`Dockerfile.frontend` + `nginx.conf`) y PocketID. El backend **no se publica al host**: nginx es el único ingress, sirve el SPA y hace proxy inverso de `/api` hacia `backend:3000`.

```bash
# Variables requeridas
export OPENROUTER_API_KEY="sk-..."
export AUTH_ISSUER_URL="https://auth.example.com"          # issuer público, coincide con el claim `iss` del id_token
export AUTH_CLIENT_ID="valet"
export AUTH_CLIENT_SECRET="..."
export AUTH_REDIRECT_URL="https://app.example.com/api/auth/callback"
export AUTH_POST_LOGOUT_REDIRECT_URL="https://app.example.com"
export JWT_SECRET="cambiar-en-produccion"
export POCKETID_URL="https://auth.example.com"             # PocketID PUBLIC_APP_URL

# Levantar (la auth va activa por defecto; exporta AUTH_ENABLED=false para arrancar sin PocketID)
podman compose -f docker-compose.prod.yml up -d
# App: http://<host>/
```

### Cliente OIDC en PocketID

Crea un cliente OIDC en PocketID y registra:

- **Redirect URI** = `AUTH_REDIRECT_URL` (`https://app.example.com/api/auth/callback`)
- **Post-logout URI** = `AUTH_POST_LOGOUT_REDIRECT_URL` (`https://app.example.com`)

`AUTH_ISSUER_URL` debe ser el issuer **público** — el valor que aparece en el claim `iss` del ID token y que alcanzan tanto el navegador como el backend. El nombre de servicio interno `http://pocketid:80` *no* es un issuer válido.

### Propiedad del volumen

El backend corre como usuario no-root (UID 1000). En un volumen nombrado nuevo, Podman copia la propiedad de `/data` desde la imagen, así que es escribible. Un volumen creado por una imagen antigua (root) pertenece a `root` dentro del contenedor, y el backend no-root no puede escribir su base de datos SQLite. Reasigna la propiedad de un volumen existente con:

```bash
podman run --rm -v valet_valet_data:/data alpine chown -R 1000:1000 /data
```

**Carencia conocida: el contenedor no sobrevive a un reinicio del host.** `docker-compose.yml` no declara `restart:`, así que si la máquina se apaga el servicio se queda caído hasta levantarlo a mano — el 2026-10-01 supuso unas 9 h y media de caída. Habilitar el autoarranque está deliberadamente aplazado; el arreglo verificado está en `AGENTS.md` § V.

## 🏛️ Arquitectura

```
┌─────────────────────────────────────────────────┐
│                Frontend (SPA)                    │
│            (React + Antd + Vite)                 │
└─────────────────────┬───────────────────────────┘
                      │ HTTP/SSE
┌─────────────────────▼───────────────────────────┐
│               Rust Server (Axum)                 │
│                                                  │
│  ┌─────────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Orquestador│  │ Memoria  │  │  Tools     │  │
│  │  (ReAct)    │  │ (capas)  │  │   (14)     │  │
│  └─────────────┘  └──────────┘  └────────────┘  │
│                                                  │
│  ┌──────────────────────────────────────────┐    │
│  │         SQLite + FTS5 + sqlite-vec       │    │
│  └──────────────────────────────────────────┘    │
└──────────────────────────────────────────────────┘
```

El orquestador expone 12 herramientas desde `src/tools/`: `calendar`, `geocode`, `get_current_location`, `get_current_time`, `notes`, `reminders`, `reverse_geocode`, `search_places`, `tasks`, `unified_search`, `weather`, `web_search`.

Las herramientas con varias operaciones (`calendar`, `tasks`) declaran el permiso por operación: las lecturas se ejecutan directamente, crear y actualizar avisan, mientras que los borrados (`delete_event`, `delete_task`) pausan el turno y exigen confirmación explícita del usuario antes de ejecutarse. La confirmación se resuelve con `POST /api/approval/{request_id}`; si no llega ninguna, la operación caduca y no se ejecuta.

Workers en segundo plano en `src/workers/`: `briefing`, `collapse`, `conflict_detector`, `episodic_memory`, `memory_worker`, `pool`, `stats_cleanup`, `travel_prep`.

## 🔌 API

| Método | Ruta | Descripción |
|--------|------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/chat/stream` | Chat con streaming SSE |
| POST | `/api/approval/{request_id}` | Resolver una petición de aprobación de herramienta |
| GET | `/api/chat/init` | Estado inicial del chat |
| GET/POST | `/api/messages` | Listar / crear mensajes |
| GET | `/api/messages/{msg_id}` | Obtener un mensaje |
| GET/POST | `/api/memories` | Listar / crear memorias |
| DELETE | `/api/memories/{id}` | Borrar una memoria |
| GET/PUT | `/api/profile` | Obtener / actualizar el perfil |
| GET/PUT | `/api/settings` | Obtener / actualizar ajustes |
| GET | `/api/tools` | Listar herramientas |
| PUT | `/api/tools/{id}/toggle` | Activar / desactivar una herramienta |
| GET | `/api/search` | Búsqueda unificada |
| GET | `/api/export` | Exportar datos |
| — | `/api/events/*`, `/api/tasks/*` | CRUD de eventos y tareas |
| GET | `/api/stats/memory` | Estadísticas de memoria episódica |
| GET | `/api/stats/llm/summary` | Resumen de uso del LLM |
| GET | `/api/stats/llm/by-model`, `/api/stats/llm/by-day` | Desglose de uso del LLM |
| GET | `/api/stats/llm/tools`, `/api/stats/llm/last-call` | Uso de herramientas y última llamada |
| GET | `/api/stats/db/sizes` | Tamaños de las tablas de la BD |
| GET | `/api/stats/llm/export` | Exportar estadísticas del LLM a CSV |

## 🧪 Desarrollo y Tests

```bash
# Backend
just test          # cargo test (599 tests)
just clippy        # cargo clippy -- -D warnings
just fmt           # cargo fmt --check

# Frontend
just frontend-check   # tsc --noEmit + vite build

# Todo
just check-all
```

Recetas `just` disponibles: `dev`, `dev-docker`, `check-all`, `test`, `clippy`, `fmt`, `frontend-check`, `check-spec`, `clean`, `help`.

## 📜 Licencia

MIT
