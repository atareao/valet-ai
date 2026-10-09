# 🎩 Valet

[Español](README.es.md)

The AI personal assistant with an attitude.

Valet is a customizable, hyper-focused personal AI designed to handle your daily workflows, tasks, and queries. Built for speed and utility, it serves you with precision—and a healthy dose of dry, razor-sharp British wit.

Self-hosted and local-first: your data lives in SQLite on your own machine, and the assistant answers with the manners of a competent butler and the patience of none.

## 🗂️ Features

- **🫖 Chat with AI**: Orchestrator with a ReAct loop and layered memory (session, episodic, profile)
- **🕰️ Calendar and Tasks**: Events, tasks and reminders with `shared`/`personal` scope
- **🗺️ Weather and Geo**: Weather by coordinates, geocoding (Nominatim), place search (Overpass OSM)
- **🧐 Unified Search**: FTS5 across every dimension
- **🗝️ Private**: Local SQLite data, self-hosted, optional PocketID auth
- **🔔 Proactive**: Morning briefing, conflict detection, travel preparation
- **📱 Responsive UI**: React + Ant Design frontend that works on desktop and mobile

## 🧠 Episodic Memory

Valet keeps a two-layer episodic memory:

- **Layer A (raw messages)**: the `messages` table holds every user↔assistant interaction
- **Layer B (episodic cards)**: the `memory` table holds synthetic summaries produced by a secondary LLM

The `EpisodicMemoryWorker` fires when a message is inserted (with a 30-minute fallback timer):

1. Accumulates unindexed messages up to ~2000 tokens
2. Adds ±2 messages of overlap for context
3. Sends the block to a secondary LLM with an archivist prompt
4. Produces a structured card (`DATE`, `TOPICS`, `FACTS`, `SYNTHESIS`)
5. Stores the card plus its vector embedding in `memory` and `vec_memory`
6. Marks those messages as indexed

During chat, the orchestrator uses **RAG**: it embeds the user query, searches `vec_memory` by cosine similarity, and injects the most relevant cards into the context within a token budget.

## 🧱 Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + Axum 0.8 + Tokio + sqlx 0.9 |
| Frontend | TypeScript + React 18 + Ant Design 5 + Vite |
| Database | SQLite + FTS5 + sqlite-vec |
| Auth | PocketID (self-hosted OIDC), optional |
| LLM | OpenRouter (primary) / Ollama (fallback) |
| Containers | Docker + Docker Compose (Podman for local dev) |

## 📋 Requirements

- Rust 1.94+
- Node.js 22+
- SQLite 3.45+ (with FTS5)
- Docker or Podman + Compose (optional)

## 🚀 Quick Start

The recommended entrypoint is [`just`](https://github.com/casey/just):

```bash
# 1. Clone
git clone https://github.com/atareao/valet-ai.git
cd valet-ai

# 2. Configure
cp .env.example .env
# edit .env and add at least OPENROUTER_API_KEY

# 3. Run with Podman (rebuilds the image)
just dev
# or with Docker
just dev-docker

# Server: http://localhost:3000
```

### Running without containers

```bash
# Backend
cp .env.example .env
cargo run

# Frontend (second terminal)
cd frontend
npm install
npm run dev
```

## ⚙️ Configuration

Environment variables are read once at startup (see `src/config.rs`) with sensible defaults. The main ones (see `.env.example`):

| Variable | Description | Default |
|----------|-------------|---------|
| `HOST` | Bind address | `0.0.0.0` |
| `PORT` | HTTP port | `3000` |
| `DATABASE_URL` | Path to the SQLite database | `valet.db` |
| `LOG_LEVEL` | Log level | `info` |
| `OPENROUTER_API_KEY` | OpenRouter API key | — |
| `OPENROUTER_MODEL` | Primary chat model | `anthropic/claude-sonnet-20241022` |
| `OPENROUTER_BASE_URL` | OpenRouter API base URL | `https://openrouter.ai/api/v1` |
| `OLLAMA_BASE_URL` | Ollama fallback base URL | `http://localhost:11434` |
| `OLLAMA_MODEL` | Ollama fallback model | `llama3.2:3b` |
| `AUTH_ENABLED` | Enable PocketID authentication | `false` |
| `AUTH_ISSUER_URL` | OIDC issuer URL (required when `AUTH_ENABLED=true`) | — |
| `AUTH_CLIENT_ID` | OIDC client ID (required when `AUTH_ENABLED=true`) | — |
| `AUTH_CLIENT_SECRET` | OIDC client secret (required when `AUTH_ENABLED=true`) | — |
| `AUTH_REDIRECT_URL` | OIDC redirect URL — the PocketID client Redirect URI (`https://YOUR_DOMAIN/api/auth/callback`); required when `AUTH_ENABLED=true` | — |
| `AUTH_POST_LOGOUT_REDIRECT_URL` | Post-logout redirect URI sent to the provider (optional) | — |
| `JWT_SECRET` | Secret for signing session tokens (required when `AUTH_ENABLED=true`) | — |
| `OPENWEATHER_API_KEY` | OpenWeather API key | — |
| `GOOGLE_PLACES_API_KEY` | Google Places API key | — |
| `BRAVE_SEARCH_API_KEY` | Brave Search API key | — |
| `COLLAPSE_THRESHOLD_TOKENS` | Tokens before context collapse | `2000` |
| `COLLAPSE_MODEL` | Model used for context collapse | `mistralai/mistral-small-24b-instruct-2501` |
| `MEMORY_BATCH_TOKENS` | Accumulated tokens to trigger an episodic card | `2000` |
| `MEMORY_INACTIVITY_MINUTES` | Idle minutes before forcing a card | `30` |
| `MEMORY_OVERLAP` | Overlap messages (±) in the block | `2` |
| `MEMORY_POLL_INTERVAL_MINUTES` | Fallback timer interval | `30` |
| `MEMORY_MODEL` | Model used to generate episodic cards | `mistralai/mistral-small-24b-instruct-2501` |
| `RAG_BUDGET_TOKENS` | Token budget for chat RAG | `2000` |

## 🏭 Production

`docker-compose.prod.yml` deploys a **single service** `valet`, built from the repo's monolithic `Dockerfile` (multi-stage: the Vite SPA is compiled into `/app/static` and served by the Rust binary itself on `:3000`, together with `/api` — the same `Dockerfile` used by `docker-compose.yml` in development). **Nothing is published to the host**: the service joins an **existing Traefik** external network and is routed by labels; Traefik terminates TLS.

### The app behind Traefik

You need **one DNS name** pointing at the VPS (`APP_HOST`, **without scheme**). Traefik's external network must already exist (or point `TRAEFIK_NETWORK` at yours):

```bash
podman network create traefik
```

```bash
# Required variables
export OPENROUTER_API_KEY="sk-..."
export APP_HOST="valet.example.com"                       # app DNS name, NO scheme
export TRAEFIK_NETWORK="traefik"                          # external Traefik network
export TRAEFIK_CERT_RESOLVER="letsencrypt"                # ACME resolver configured in Traefik
export AUTH_ISSUER_URL="https://auth.example.com"         # public issuer of the existing PocketID (matches the id_token `iss` claim)
export AUTH_CLIENT_ID="valet"
export AUTH_CLIENT_SECRET="..."
export AUTH_REDIRECT_URL="https://valet.example.com/api/auth/callback"
export AUTH_POST_LOGOUT_REDIRECT_URL="https://valet.example.com"
export JWT_SECRET="change-me-in-production"

# Start (auth is enabled by default; export AUTH_ENABLED=false to bring the stack up without auth)
podman compose -f docker-compose.prod.yml up -d
# App: https://valet.example.com/
```

Traefik picks the container up from the labels and issues the certificate via `TRAEFIK_CERT_RESOLVER`; no `ports:` are exposed. The database is persisted in the `valet_data` volume mounted at `/app/data`.

### PocketID (already deployed, external)

PocketID is **not** part of this stack — it is an OIDC provider already running on the VPS. The app consumes it through `AUTH_ISSUER_URL`, which must be the **public issuer**: the value that appears in the ID token `iss` claim and is reachable by both the browser and the backend (discovery, code exchange and JWKS all resolve against it). In the existing PocketID, register the OIDC client with:

- **Redirect URI** = `https://${APP_HOST}/api/auth/callback` (i.e. `AUTH_REDIRECT_URL`)
- **Post-logout URI** = `https://${APP_HOST}` (i.e. `AUTH_POST_LOGOUT_REDIRECT_URL`)

No PocketID container, volume or DNS name is created by this repo.

### Data volume

The database lives in the named volume `valet_data` mounted at `/app/data` (see `DATABASE_URL`). On a fresh volume Podman seeds its ownership from the image. The app runs with whatever user the image defines — the monolithic `Dockerfile` runs as `root`, matching the development image.

**Known gap: the container does not survive a host reboot.** `docker-compose.yml` omits `restart:`, so if the machine goes down the service stays down until it is started by hand.

## 🏛️ Architecture

```
┌─────────────────────────────────────────────────┐
│                 Frontend (SPA)                   │
│            (React + Antd + Vite)                 │
└─────────────────────┬───────────────────────────┘
                      │ HTTP/SSE
┌─────────────────────▼───────────────────────────┐
│               Rust Server (Axum)                 │
│                                                  │
│  ┌─────────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Orchestrator│  │  Memory  │  │   Tools    │  │
│  │   (ReAct)   │  │ (layered)│  │   (14)     │  │
│  └─────────────┘  └──────────┘  └────────────┘  │
│                                                  │
│  ┌──────────────────────────────────────────┐    │
│  │         SQLite + FTS5 + sqlite-vec       │    │
│  └──────────────────────────────────────────┘    │
└──────────────────────────────────────────────────┘
```

The orchestrator exposes 12 tools from `src/tools/`: `calendar`, `geocode`, `get_current_location`, `get_current_time`, `notes`, `reminders`, `reverse_geocode`, `search_places`, `tasks`, `unified_search`, `weather`, `web_search`.

Multi-operation tools (`calendar`, `tasks`) declare their permission per operation: reads run directly, creates and updates notify, while destructive deletes (`delete_event`, `delete_task`) pause the turn and require explicit user confirmation before running. A confirmation is resolved via `POST /api/approval/{request_id}`; if none arrives, the operation times out and is not executed.

Background workers in `src/workers/`: `briefing`, `collapse`, `conflict_detector`, `episodic_memory`, `memory_worker`, `pool`, `stats_cleanup`, `travel_prep`.

## 🔌 API

| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/chat/stream` | Chat with SSE streaming |
| POST | `/api/approval/{request_id}` | Resolve a tool approval request |
| GET | `/api/chat/init` | Initial chat state |
| GET/POST | `/api/messages` | List / create messages |
| GET | `/api/messages/{msg_id}` | Get a message |
| GET/POST | `/api/memories` | List / create memories |
| DELETE | `/api/memories/{id}` | Delete a memory |
| GET/PUT | `/api/profile` | Get / update the profile |
| GET/PUT | `/api/settings` | Get / update settings |
| GET | `/api/tools` | List tools |
| PUT | `/api/tools/{id}/toggle` | Enable / disable a tool |
| GET | `/api/search` | Unified search |
| GET | `/api/export` | Export data |
| — | `/api/events/*`, `/api/tasks/*` | Events and tasks CRUD |
| GET | `/api/stats/memory` | Episodic memory statistics |
| GET | `/api/stats/llm/summary` | LLM usage summary |
| GET | `/api/stats/llm/by-model`, `/api/stats/llm/by-day` | LLM usage breakdowns |
| GET | `/api/stats/llm/tools`, `/api/stats/llm/last-call` | Tool usage and last call |
| GET | `/api/stats/db/sizes` | Database table sizes |
| GET | `/api/stats/llm/export` | Export LLM stats as CSV |

## 🧪 Development and Tests

```bash
# Backend
just test          # cargo test (599 tests)
just clippy        # cargo clippy -- -D warnings
just fmt           # cargo fmt --check

# Frontend
just frontend-check   # tsc --noEmit + vite build

# Everything
just check-all
```

Available `just` recipes: `dev`, `dev-docker`, `check-all`, `test`, `clippy`, `fmt`, `frontend-check`, `check-spec`, `clean`, `help`.

## 📜 License

MIT
