# PLAN-025 — GitHub: crear issues desde Valet

- **Issue de entrada:** [#333](https://github.com/atareao/valet-ai/issues/333) (asignado a `atareao`)
- **Change de OpenSpec:** `openspec/changes/github-integration/`
- **Estado:** spec redactada; **pendiente de aprobación** antes de TDD

## Objetivo

Que el usuario pueda **crear issues en GitHub por conversación** («ábreme un issue en valet-ai para
X»), configurando el token desde **Ajustes → Integraciones → GitHub**, sin tocar el servidor.

## Diagnóstico (verificado)

- Valet no tiene ninguna integración con GitHub: `rg github src/` solo aparece en los clientes de
  LLM/embeddings.
- No hay servicio, herramienta, ajuste ni skill de GitHub.

## Alcance (decisiones del usuario)

- **Token: Personal Access Token clásico**, cabecera `Authorization: Bearer <token>` (el issue
  recomendaba fine-grained; se eligió clásico).
- **Una sola operación: `create_issue`** (sin listar ni buscar).
- **Repo destino:** ajuste `github_default_repo` (no sensible) + override por parámetro `repo`; sin
  ninguno, la herramienta falla con mensaje accionable **sin tocar la red**.
- **Labels:** ajuste `github_default_labels` (no sensible, lista con comas) con override por
  parámetro `labels`.
- `github_token` **sensible**; `github_default_repo` y `github_default_labels` no sensibles.
- Permiso de `create_issue`: **`Notify`** (no destructiva).

## Cambios

1. Nuevos ajustes: `github_default_repo` (def. `atareao/valet-ai`), `github_default_labels` (vacío),
   `github_token` (sensible).
2. `github_token` en `SENSITIVE_KEYS`; `GET /api/github/status` → `{configured, default_repo}` sin el
   token; `PUT /api/github/token` (400 vacío / 204 ok).
3. Sonda `GET /api/github/check` (siempre 200 `{ok, detail}`), molde `/api/blog/check`, consultando
   `GET https://api.github.com/user`.
4. `src/services/github.rs`: cliente `create_issue` contra `POST /repos/{repo}/issues`, sin reintentos,
   solo `201` como éxito, traducción de errores tipados sin filtrar el token.
5. `src/tools/github.rs`: herramienta multioperación `github` con validación de args sin red.
6. Skill `github` en el catálogo (umbral `0.10`, `SKILL_GITHUB_PROMPT`, `# SKILL ACTIVA: GITHUB`); el
   catálogo pasa a catorce skills y el registry a treinta y cinco herramientas.
7. Migración aditiva e idempotente `migrations/<timestamp>_github.sql`.
8. Frontend: sub-pestaña «GitHub» (`GithubSection.tsx`), token siempre vacío, «Probar conexión»; el
   recuento de sub-pestañas pasa de diez a once.

## Verificación

- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- `vitest run`, `npm run lint`, `npx tsc --noEmit` en `frontend/`.
- `openspec validate github-integration --strict`.
- Auditoría `rust-reviewer` + `react-reviewer`.
- **E2E real:** configurar un PAT clásico en Ajustes; `check` → `{ok:true}`; crear un issue real en
  `atareao/valet-ai` y comprobarlo.

## Follow-ups

- Valorar operaciones de listado/búsqueda de issues y operaciones destructivas (cerrar/editar) en un
  change futuro, con `ExplicitApproval` cuando proceda.
- Documentar la integración en `docs/github-integration.md` y añadir la fila al índice de `plans/`.
