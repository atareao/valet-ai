# Tasks

## 1. RED — caracterización y tests que fallan

- [ ] 1.1 Test de migración: `llm_requests` gana `kind TEXT NOT NULL DEFAULT 'chat'` con el `CHECK` de los cinco valores; las filas previas quedan `'chat'`; reejecutar migraciones no falla.
- [ ] 1.2 Test de `record_request` con `kind`: inserta la fila con el origen dado; el chat pasa `Chat`, el archivist `Archivist`, el consolidator `Consolidator`, el collapse `Collapse`.
- [ ] 1.3 Tests de agregaciones de chat: con filas de varios orígenes, `summary`/`by_model`/`by_day`/`tools_summary` **ignoran** todo lo que no sea `kind='chat'`.
- [ ] 1.4 Test de `background_summary`: devuelve una entrada por origen no-chat (calls, input/output tokens, coste, errores, latencia media); los orígenes sin filas salen a cero.
- [ ] 1.5 Test de `export_csv`: la cabecera incluye `kind` y cada línea lo lleva.
- [ ] 1.6 Test de orquestador: un turno enrutado inserta una fila `kind='router'`; un fallo del clasificador inserta `status='error'`; `Disabled` no inserta nada; `last_api_call` no cambia.
- [ ] 1.7 Test de handler `GET /api/stats/llm/background`.
- [ ] 1.8 Frontend: `BackgroundCard.test.tsx` (una fila por origen y estado vacío).

## 2. GREEN — implementación mínima

- [ ] 2.1 Migración `20261009000001_llm_requests_kind.sql`.
- [ ] 2.2 `src/models/stats.rs`: `CallKind` y `BackgroundStats`.
- [ ] 2.3 `StatsRepo::record_request` con `kind`; filtros `kind='chat'` en las agregaciones de chat; `background_summary`; `kind` en `export_csv`.
- [ ] 2.4 Actualizar los puntos de llamada: chat (`agent.rs`), archivist/consolidator (`episodic_memory.rs`), collapse (`collapse.rs`) y los dos puntos muertos de `analyze`.
- [ ] 2.5 `skill_router.rs`: `RouterUsage` y `usage: Option<RouterUsage>` en `Selection` (poblado en `Router` y `Error`).
- [ ] 2.6 `agent.rs`: persistir la fila `router` tras `select` (y solo entonces), sin tocar `last_api_call`.
- [ ] 2.7 Handler `background_handler` + ruta `GET /api/stats/llm/background`.
- [ ] 2.8 Frontend: tipo `BackgroundStats`, `api.getStatsBackground()`, `BackgroundCard.tsx`, tarjeta en la pestaña «Modelos».

## 3. REFACTOR y verificación

- [ ] 3.1 `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- [ ] 3.2 Frontend: `npm run typecheck`, `npm run lint`, `npm test`.
- [ ] 3.3 `rust-reviewer` y `react-reviewer` sobre el cambio.
- [ ] 3.4 Verificación E2E en instancia efímera: un turno enrutado crea fila `kind='router'`; los workers crean sus filas; el panel de chat no cambia; el endpoint devuelve una entrada por origen.
