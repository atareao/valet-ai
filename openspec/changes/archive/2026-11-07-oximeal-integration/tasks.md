# Tasks: oximeal-integration

## 1. RED (tests que fallan)

### 1.1 Servicio `Oximeal` (`src/services/oximeal.rs`)
- [x] 1.1.1 Test: `Debug` de `OximealConfig` no contiene el secreto.
- [x] 1.1.2 Test: sin `client_id`/`client_secret` → `NotConfigured` sin tocar la red.
- [x] 1.1.3 Test: `oximeal_base_url` por defecto es `https://oximeal.territoriolinux.es`.
- [x] 1.1.4 Test (`wiremock`): descubre `token_endpoint` desde `<issuer>/.well-known/openid-configuration`.
- [x] 1.1.5 Test (`wiremock`): la petición `client_credentials` lleva `client_id`, `client_secret` y `scope=agent:read agent:plan agent:shopping agent:write`.
- [x] 1.1.6 Test (`wiremock`): el token se cachea y **no** se vuelve a pedir antes de expirar.
- [x] 1.1.7 Test (`wiremock`): expirado el token (margen 30 s), se pide uno nuevo.
- [x] 1.1.8 Test (`wiremock`): `401`/`403` de la Agent API → `Unauthorized` e invalidación de la caché.
- [x] 1.1.9 Test (`wiremock`): `5xx`/fallo de red → `Upstream`.
- [x] 1.1.10 Test (`wiremock`): una operación por cada endpoint (`recent_recipes`, `search_recipes`, `set_weekly_plan`, `add_shopping_items`, `delete_completed`, `create_recipe`) con su consulta/cuerpo correcto y el mapeo de la respuesta.
- [x] 1.1.11 Test (`wiremock`): `400` del servidor → `InvalidInput` con el mensaje del servidor; `404` → `NotFound`.
- [x] 1.1.12 `cargo test` → RED.

### 1.2 Herramienta `oximeal` (`src/tools/oximeal.rs`)
- [x] 1.2.1 Test: `name() == "oximeal"` y `parameters()` es un esquema JSON válido con `operation` en el enum y `required: ["operation"]`.
- [x] 1.2.2 Test: validación de argumentos **sin red** — `date` no `YYYY-MM-DD`, `meal_type`/`role`/`category` fuera de enum, `servings < 1`, `name` vacío, `amount < 0`, `entries`/`items`/`ingredients` vacíos → `InvalidArguments`.
- [x] 1.2.3 Test: `permission()` por operación (`recent_recipes`/`search_recipes` → `NoConfirm`; `add_shopping_items`/`create_recipe` → `Notify`; `plan_week`/`clear_purchased` → `ExplicitApproval`).
- [x] 1.2.4 Test: traducción de errores a mensajes accionables en español, sin credenciales.
- [x] 1.2.5 `cargo test` → RED.

### 1.3 Rutas y handlers (`src/handlers/oximeal.rs`)
- [x] 1.3.1 Test: `status` informa `configured`, `base_url`, `issuer_url` y `has_client_id`, y **nunca** el secreto.
- [x] 1.3.2 Test: `check` siempre `200` con los detalles «configuración incompleta», «el IdP no entregó token», «el token fue rechazado», «OxiMeal no responde» y «conexión correcta» (con `wiremock`).
- [x] 1.3.3 Test: `update_secret` con valor vacío/ausente → `400`; con valor válido → guardado recortado y `204`.
- [x] 1.3.4 Test: `oximeal_client_secret` está en `SENSITIVE_KEYS` y `GET /api/settings` lo omite.
- [x] 1.3.5 `cargo test` → RED.

### 1.4 Catálogo de skills y registry
- [x] 1.4.1 Test: el catálogo declara **trece** skills (incluye `menus`) y cubre todas las herramientas.
- [x] 1.4.2 Test: el registry registra **treinta y cuatro** herramientas (incluye `oximeal`), sin duplicados.
- [x] 1.4.3 `cargo test` → RED.

### 1.5 Frontend
- [x] 1.5.1 Test `useOximeal.test.ts`: estados `loading`/`configured`/`degraded`, `check` y que el secreto no se relee.
- [x] 1.5.2 Test `OximealSection.test.tsx`: campos de base/issuer/client_id, campo del secreto **siempre vacío al abrir**, botón «Probar conexión».
- [x] 1.5.3 `npx vitest run` → RED.

## 2. GREEN (implementación mínima)
- [x] 2.1 Implementar `src/services/oximeal.rs` (`Oximeal`, `OximealConfig`, `OximealError`, `resolve_config`, token M2M + caché, seis operaciones) y `pub mod oximeal;` en `src/services/mod.rs`.
- [x] 2.2 Implementar `src/tools/oximeal.rs` (`OximealTool`) y `pub mod oximeal;` en `src/tools/mod.rs`.
- [x] 2.3 Registrar la herramienta en `build_tool_registry()` de `src/lib.rs`.
- [x] 2.4 Implementar `src/handlers/oximeal.rs` + `src/routes/oximeal.rs`, declarar ambos módulos y registrar `.merge(routes::oximeal::routes())` en `src/lib.rs`.
- [x] 2.5 Añadir `oximeal_client_secret` a `SENSITIVE_KEYS` en `src/db/repos/settings.rs`.
- [x] 2.6 Añadir `Skill::Menus` al enum, al `CATALOG` (`tools: &["oximeal"]`, `prompt_key: "SKILL_MENUS_PROMPT"`, `prompt_heading: "# SKILL ACTIVA: MENUS"`, `threshold: 0.10`) y a `ALL_SKILLS` en `src/orchestrator/skills.rs`.
- [x] 2.7 Migración nueva: sembrar `oximeal_base_url`, `oximeal_client_id` y `SKILL_MENUS_PROMPT` (texto por defecto; conservar `# SKILL ACTIVA: MENUS` como primera línea).
- [x] 2.8 Implementar `frontend/src/hooks/useOximeal.ts` + `frontend/src/components/OximealSection.tsx` y su entrada en la pestaña «Integraciones» de `SettingsDialog.tsx`.
- [x] 2.9 Todas las puertas en verde: `cargo test`, `npx vitest run`, `npx tsc --noEmit`, `npm run lint`.

## 3. REFACTOR
- [x] 3.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- [x] 3.2 Documentar las variables `OXIMEAL_*` en `.env.example` y el prerrequisito del cliente M2M en PocketID.
- [x] 3.3 Añadir la fila del `PLAN-021` al índice de `plans/` en `AGENTS.md`.
- [x] 3.4 `just check-all` y `openspec validate --all --strict` en verde.
- [x] 3.5 Auditoría: `@rust-reviewer` (servicio/tool/rutas) y `@react-reviewer` (UI de ajustes); enrutar hallazgos.

## 4. Cierre
- [ ] 4.1 Verificación E2E contra un OxiMeal real con cliente M2M: `check` → `{ok:true}`; `search_recipes`; `plan_week` (devuelve `shopping_list`); `add_shopping_items`; `clear_purchased`; `create_recipe` y su aparición posterior en `search_recipes`. **Requiere despliegue de OxiMeal + cliente M2M (fuera de alcance): si no se dispone, se documenta como pendiente.** <!-- PENDIENTE: OxiMeal no está desplegado en co1; requiere cliente M2M real. Ver docs/oximeal-integration.md -->
- [x] 4.2 Verificar que `GET /api/settings` no contiene `oximeal_client_secret` ni ningún log lo filtra.
- [x] 4.3 `openspec archive oximeal-integration`. <!-- Se completa en el cierre, a continuación -->
- [x] 4.4 Gitflow: `feature/oximeal-integration`, commits gitmoji, PR a `development`. <!-- Se completa en el cierre, a continuación -->
