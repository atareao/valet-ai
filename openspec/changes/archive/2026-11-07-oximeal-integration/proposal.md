# Change: oximeal-integration

## Why

OxiMeal (`/data/rust/oximeal`) es un gestor *self-hosted* de menús semanales, recetas y listas de la compra, **headless y API-first**: expone una **Agent API** bajo `/api/agent` diseñada explícitamente para que **Valet** la consuma (ADR-5/ADR-9 de su `PLAN.md`, `backend/src/routes/agent.rs`, spec `agent-api`). Hoy Valet **no** puede leer recetas, planificar el menú semanal, gestionar la lista de la compra ni guardar recetas de OxiMeal: la misma carencia que cubrieron `zennotes-notes` y `opentodo-integration` para sus respectivos servicios.

Valet ya dispone del molde de integración nativa (`services/<svc>.rs` + `tools/<svc>.rs` + `routes`/`handlers` + ajustes sensibles + catálogo de skills + sección de ajustes en el frontend), de modo que añadir OxiMeal es replicar ese patrón con sus particularidades: **la Agent API no acepta API key estática** (obsoleta y retirada) y autentica por **OIDC M2M** (`client_credentials` contra PocketID → `Authorization: Bearer`, scopes `agent:*`).

## What Changes

- **Servicio `Oximeal`** (`src/services/oximeal.rs`): cliente HTTP que resuelve su configuración en **cada** llamada, obtiene y cachea un **token M2M** (`client_credentials` contra el `token_endpoint` descubierto del emisor OIDC) y expone las seis operaciones de la Agent API. El secreto nunca aparece en `Debug`, logs ni mensajes de error.
- **Herramienta `oximeal`** (`src/tools/oximeal.rs`): herramienta **multioperación** (`recent_recipes`, `search_recipes`, `plan_week`, `add_shopping_items`, `clear_purchased`, `create_recipe`) con validación de argumentos **sin salir a la red** y permiso por operación.
- **Skill `menus`** en el catálogo cerrado (`src/orchestrator/skills.rs`), que agrupa la herramienta `oximeal`. El catálogo pasa de **doce a trece** skills y el registry de **treinta y tres a treinta y cuatro** herramientas.
- **API de integración de Valet**: `GET /api/oximeal/status`, `GET /api/oximeal/check` (sonda real) y `PUT /api/oximeal/secret`. `oximeal_client_secret` entra en `SENSITIVE_KEYS`.
- **Migración** que siembra `oximeal_base_url`, `oximeal_client_id` y el fragmento `SKILL_MENUS_PROMPT`.
- **Frontend**: sección «OxiMeal» en Ajustes → Integraciones, con sonda de conexión real.
- **Sin despliegue de OxiMeal** en este change y **sin** cambios en `docker-compose*.yml`.

## Decisions

- **D1** — Una única herramienta `oximeal` multioperación (parámetro `operation`), molde `zennotes`/`calendar`. No se crean seis herramientas.
- **D2** — Skill nueva `menus` (id `menus`, prompt `SKILL_MENUS_PROMPT`, umbral por defecto `0.10`).
- **D3** — `oximeal_base_url`, `oximeal_issuer_url` y `oximeal_client_id` NO sensibles; `oximeal_client_secret` **SENSIBLE**.
- **D4** — Token M2M cacheado en memoria de proceso con margen de 30 s; descubrimiento del `token_endpoint` cacheado; configuración releída en cada llamada.
- **D5** — `GET /api/oximeal/check` es la sonda **real** (obtiene token y consulta la Agent API), siempre `200` con `{ok, detail}`.
- **D6** — Permisos por operación: lecturas `NoConfirm`; `add_shopping_items` y `create_recipe` `Notify`; `plan_week` y `clear_purchased` `ExplicitApproval`.
- **D7** — Solo autenticación M2M: no se implementa el modo abierto (`OIDC_ENABLED=false`) en el cliente de Valet.

## Impact

- **Specs**: nuevas capacidades `oximeal-service` y `oximeal-api`; MODIFIED en `orchestrator/skill-router` (catálogo de skills) y `tools/registry` (registro de herramientas); ADDED en `frontend` (sección de ajustes).
- **Código backend**: `src/services/{oximeal,mod}.rs`, `src/tools/{oximeal,mod}.rs`, `src/routes/{oximeal,mod}.rs`, `src/handlers/{oximeal,mod}.rs`, `src/db/repos/settings.rs`, `src/orchestrator/skills.rs`, `src/lib.rs` y una migración nueva.
- **Código frontend**: `frontend/src/components/OximealSection.tsx`, `frontend/src/hooks/useOximeal.ts`, `frontend/src/components/SettingsDialog.tsx` y sus tests.
- **Base de datos**: una migración aditiva (ajustes y fragmento de prompt). Sin cambios de esquema.
- **Dependencias**: ninguna nueva (`reqwest` y `serde` ya están; `wiremock` ya en `dev-dependencies`).
- **Fuera de alcance**: despliegue de OxiMeal, alta del cliente M2M en PocketID, modo abierto del cliente, importación de recetas por URL y cualquier cambio en `docker-compose*.yml`.
