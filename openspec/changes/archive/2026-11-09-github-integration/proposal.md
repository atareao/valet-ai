# Change: github-integration

## Why

Valet no tiene hoy ninguna integración con **GitHub**: `rg github src/` solo aparece en los clientes de LLM/embeddings, no como servicio, herramienta, ajuste ni skill. El usuario quiere crear issues **por conversación** («ábreme un issue en valet-ai para X») contra sus repositorios, configurando el token desde Ajustes → Integraciones, igual que ya hacen ZenNotes, OpenTodo, OxiMeal y el blog. Issue [#333](https://github.com/atareao/valet-ai/issues/333).

## What Changes

- **Servicio `Github`** (`src/services/github.rs`): cliente HTTP que resuelve su configuración **en cada** llamada desde `settings` y expone `create_issue` contra `POST https://api.github.com/repos/{repo}/issues`. El token viaja en `Authorization: Bearer <token>` y **nunca** aparece en `Debug`, logs ni mensajes de error.
- **Herramienta `github`** (`src/tools/github.rs`): herramienta **multioperación** con la única operación `create_issue`, con validación de argumentos **sin salir a la red** y permiso `Notify`.
- **Skill `github`** en el catálogo cerrado (`src/orchestrator/skills.rs`), que agrupa la herramienta `github`. El catálogo pasa de **trece a catorce** skills y el registry de **treinta y cuatro a treinta y cinco** herramientas.
- **API de integración de Valet**: `GET /api/github/status` → `{configured, default_repo}`, la sonda real `GET /api/github/check` (siempre `200` con `{ok, detail}`) y la puerta dedicada `PUT /api/github/token`. `github_token` entra en `SENSITIVE_KEYS`.
- **Ajustes**: `github_token` (**sensible**), `github_default_repo` y `github_default_labels` (no sensibles, lista separada por comas).
- **Migración** aditiva e idempotente que siembra `github_default_repo`, `github_default_labels` y el fragmento `SKILL_GITHUB_PROMPT` (no siembra el token).
- **Frontend**: sección «GitHub» en Ajustes → Integraciones (token que arranca **siempre vacío**, botón «Probar conexión» contra `/check`) y actualización del orden y del recuento de sub-pestañas (de **diez a once**).
- **Sin cambios** en `docker-compose*.yml` ni en el despliegue.

## Capabilities

### New Capabilities
- `github`: contrato del servicio y de la herramienta — resolución de la configuración en cada llamada, autenticación `Bearer`, resolución del repositorio (ajuste + override por parámetro), labels, creación del issue con `POST /repos/{repo}/issues` y traducción de `401/403/404/422/429/5xx` a errores tipados sin filtrar el token.
- `github-api`: superficie HTTP de la integración GitHub —sensibilidad del token, endpoints dedicados (`GET /api/github/status`, `PUT /api/github/token`), sonda real (`GET /api/github/check`) y siembra de ajustes y prompt por migración.

### Modified Capabilities
- `frontend`: nueva sub-pestaña «GitHub» en «Integraciones» y actualización del orden y del recuento de sub-pestañas y de sus indicadores.
- `orchestrator/skill-router`: el catálogo cerrado pasa a catorce skills (incluye `github`) y el registry a treinta y cinco herramientas.
- `tools/registry`: el registry de producción registra treinta y cinco herramientas (incluye `github`).

## Impact

- **Specs**: nuevas capacidades `github` y `github-api`; MODIFIED en `frontend`, `orchestrator/skill-router` y `tools/registry`.
- **Código backend**: `src/services/{github,mod}.rs`, `src/tools/{github,mod}.rs`, `src/routes/{github,mod}.rs`, `src/handlers/{github,mod}.rs`, `src/db/repos/settings.rs` (`SENSITIVE_KEYS`), `src/orchestrator/skills.rs`, `src/lib.rs` y una migración nueva.
- **Código frontend**: `frontend/src/components/GithubSection.tsx`, `frontend/src/hooks/useGithub.ts`, `frontend/src/components/IntegrationsTab.tsx`, `frontend/src/api/client.ts`, los tipos y sus tests.
- **Base de datos**: una migración aditiva (ajustes y fragmento de prompt). Sin cambios de esquema.
- **Dependencias**: ninguna nueva (`reqwest` y `serde` ya están; `wiremock` ya en `dev-dependencies`).
- **Fuera de alcance**: listar/buscar issues, operaciones destructivas, autenticación por GitHub App/OAuth, token fine-grained (se usa un **Personal Access Token clásico**), reintentos y cambios en `docker-compose*.yml`.
