# Tasks: github-integration

## 1. RED (tests que fallan)

### 1.1 Servicio `Github` (`src/services/github.rs`)
- [x] 1.1.1 Test: `Debug` de `GithubConfig` no contiene el token.
- [x] 1.1.2 Test: sin `github_token` → `NotConfigured` sin tocar la red.
- [x] 1.1.3 Test: la resolución lee `github_default_repo` y parsea `github_default_labels` (coma).
- [x] 1.1.4 Test: `github_default_repo` por defecto es `atareao/valet-ai`.
- [x] 1.1.5 Test (`wiremock`): `POST /repos/{repo}/issues` lleva `Authorization: Bearer` y el cuerpo `{title, body?, labels?}`.
- [x] 1.1.6 Test (`wiremock`): `201` → éxito con el número y la URL del issue; cuerpo sin `body`/`labels` cuando faltan.
- [x] 1.1.7 Test (`wiremock`): `401`/`403` → `Unauthorized`; `404` → `NotFound`; `422` → `InvalidInput` con el mensaje del servidor; `429` → `RateLimited`; `5xx`/red → `Upstream`.
- [x] 1.1.8 Test: no hay reintentos (una sola petición registrada por llamada).
- [x] 1.1.9 Test (`wiremock`): el token nunca aparece en el `Display` de ningún error.
- [x] 1.1.10 `cargo test` → RED.

### 1.2 Herramienta `github` (`src/tools/github.rs`)
- [x] 1.2.1 Test: `name() == "github"` y `parameters()` es un esquema JSON válido con `operation` en el enum y `required: ["operation"]`.
- [x] 1.2.2 Test: validación **sin red** — `title` vacío, `repo` sin forma `owner/name`, labels vacíos → `InvalidArguments`.
- [x] 1.2.3 Test: resolución del repo (`repo` > `github_default_repo`) y de los labels (`labels` > `github_default_labels`).
- [x] 1.2.4 Test: sin `repo` ni default → falla con mensaje accionable sin petición HTTP.
- [x] 1.2.5 Test: `permission()` de `create_issue` es `Notify`.
- [x] 1.2.6 Test: traducción de errores a mensajes accionables en español, sin el token.
- [x] 1.2.7 `cargo test` → RED.

### 1.3 Rutas y handlers (`src/handlers/github.rs`)
- [x] 1.3.1 Test: `GET /api/github/status` informa `configured` y `default_repo`, y **nunca** el token.
- [x] 1.3.2 Test: `GET /api/github/check` siempre `200` con los detalles «configuración incompleta», «el token fue rechazado», «GitHub no responde» y «conexión correcta» (`wiremock`).
- [x] 1.3.3 Test: `PUT /api/github/token` con valor vacío/ausente → `400`; con valor válido → guardado recortado y `204`.
- [x] 1.3.4 Test: `github_token` está en `SENSITIVE_KEYS`, `GET /api/settings` lo omite y `PUT /api/settings` lo ignora; `github_default_repo` y `github_default_labels` viajan.
- [x] 1.3.5 `cargo test` → RED.

### 1.4 Migración, catálogo de skills y registry
- [x] 1.4.1 Test: la migración siembra `github_default_repo` (`atareao/valet-ai`), `github_default_labels` (vacío) y `SKILL_GITHUB_PROMPT` (primera línea `# SKILL ACTIVA: GITHUB`), sin pisar valores del usuario y de forma idempotente.
- [x] 1.4.2 Test: el catálogo declara **catorce** skills (incluye `github`) y cubre todas las herramientas.
- [x] 1.4.3 Test: el registry registra **treinta y cinco** herramientas (incluye `github`), sin duplicados.
- [x] 1.4.4 `cargo test` → RED.

### 1.5 Frontend
- [x] 1.5.1 Test `useGithub.test.ts`: estados `loading`/`configured`/`degraded`, `check` y que el token no se relee.
- [x] 1.5.2 Test `GithubSection.test.tsx`: campos de repo y labels, campo del token **siempre vacío al abrir**, botón «Probar conexión».
- [x] 1.5.3 Test `SettingsDialog.test.tsx`: hay **once** sub-pestañas y la nueva «GitHub» está en orden, con su indicador.
- [x] 1.5.4 `npx vitest run` → RED.

## 2. GREEN (implementación mínima)
- [x] 2.1 Implementar `src/services/github.rs` (`Github`, `GithubConfig`, `GithubError`, `resolve_config`, `create_issue`) y `pub mod github;` en `src/services/mod.rs`.
- [x] 2.2 Implementar `src/tools/github.rs` (`GithubTool`) y `pub mod github;` en `src/tools/mod.rs`.
- [x] 2.3 Registrar la herramienta en `build_tool_registry()` de `src/lib.rs`.
- [x] 2.4 Implementar `src/handlers/github.rs` + `src/routes/github.rs`, declarar ambos módulos y registrar `.merge(routes::github::routes())` en `src/lib.rs`.
- [x] 2.5 Añadir `github_token` a `SENSITIVE_KEYS` en `src/db/repos/settings.rs`.
- [x] 2.6 Añadir `Skill::Github` al enum, al `CATALOG` (`tools: &["github"]`, `prompt_key: "SKILL_GITHUB_PROMPT"`, `prompt_heading: "# SKILL ACTIVA: GITHUB"`, `threshold: 0.10`) y a `ALL_SKILLS` en `src/orchestrator/skills.rs`.
- [x] 2.7 Migración nueva: sembrar `github_default_repo`, `github_default_labels` y `SKILL_GITHUB_PROMPT` (texto por defecto; conservar `# SKILL ACTIVA: GITHUB` como primera línea).
- [x] 2.8 Implementar `frontend/src/hooks/useGithub.ts` + `frontend/src/components/GithubSection.tsx`, los métodos del cliente (`getGithubStatus`, `setGithubToken`, `checkGithub`) y su entrada en `IntegrationsTab.tsx`.
- [x] 2.9 Todas las puertas en verde: `cargo test`, `npx vitest run`, `npx tsc --noEmit`, `npm run lint`. (738 tests vitest verdes; tsc y eslint limpios)

## 3. REFACTOR
- [x] 3.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- [x] 3.2 Documentar la integración en `docs/github-integration.md`.
- [x] 3.3 Añadir la fila del `PLAN-025` al índice de `plans/` en `AGENTS.md`.
- [x] 3.4 `just check-all` y `openspec validate --all --strict` en verde. (node 22: `just check-all` verde — 2240 tests Rust, 738 vitest, bundle inicial 69.98 kB/70.00 kB; `openspec validate --all --strict` → 66/66)
- [x] 3.5 Auditoría: `@rust-reviewer` (servicio/tool/rutas) y `@react-reviewer` (UI de ajustes); enrutar hallazgos. (`rust-reviewer` ⚠️ y `react-reviewer` ⚠️, sin bloqueantes; hallazgos aplicados: rename del test a *fourteen*, `validate_repo` rechaza espacios internos, `optional_str`/`required_str` type-aware, helper `setting()` deduplicado, sonda inyectable con asserts de que el token no se filtra, `refetch` muerto retirado y aserción del estado «Sin datos». Además, `frontend/src/api/github.ts` extraído para que los métodos de GitHub NO entren en el grafo inicial del bundle)

## 4. Cierre
- [x] 4.1 Verificación E2E contra GitHub real con un PAT clásico: `check` → `{ok:true}`; `create_issue` contra `atareao/valet-ai` y comprobación del issue creado.
- [x] 4.2 Verificar que `GET /api/settings` no contiene `github_token` ni ningún log lo filtra.
- [x] 4.3 `openspec archive github-integration`.
- [x] 4.4 Gitflow: `feature/github-integration`, commits gitmoji, PR a `development`.
