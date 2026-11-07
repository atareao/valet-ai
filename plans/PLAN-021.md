# PLAN-021 — Integración de OxiMeal en Valet (menús de la semana, recetas y lista de la compra)

**Estado:** borrador — pendiente de aprobación de la spec (fase SDD).
**Fecha:** 2026-11-06.
**Proyecto anfitrión:** Valet (`/data/rust/valet`).
**Proyecto consumido:** OxiMeal (`/data/rust/oximeal`) — **no se modifica** en este plan.

---

## 0. Reconocimiento (hallazgos verificados)

### 0.1 Qué es OxiMeal

Gestor *self-hosted* de menús semanales, recetas y listas de la compra (Axum + SQLite, binario único). Es **headless / API-first**: no tiene orquestador LLM ni chat. Su asistente conversacional designado es **Valet**, al que sirve mediante una **Agent API** bajo `/api/agent`, pensada explícitamente como «contrato HTTP estable que consume el asistente Valet» (ADR-5 y ADR-9 de su `PLAN.md`).

Ficheros que lo demuestran en el repo de OxiMeal:
- `backend/src/routes/agent.rs` — la Agent API.
- `backend/src/models/agent.rs` — los DTOs del contrato.
- `openspec/specs/agent-api/spec.md` — spec de la capacidad.
- `README.md` §«Permisos por endpoint» y §«Migración del API key estático».

### 0.2 Agent API disponible (contrato real, leído del código)

| Endpoint | Cuerpo / query | Respuesta | Permiso |
|---|---|---|---|
| `GET /api/agent/recent-recipes` | `?days=<u32>` (def. 7, máx 365) | `{recipes:[{id,name,category,last_planned}]}` | `agent:read` |
| `GET /api/agent/search-recipes` | `?query=<str>&category=<str>` (máx 10 resultados; FTS5 si ≥3 caracteres, LIKE si no) | `{recipes:[RecipeSummary]}` | `agent:read` |
| `POST /api/agent/weekly-plan` | `{entries:[{date:"YYYY-MM-DD", meal_type:"lunch|dinner|breakfast|snack", recipe_id, role:"main|salad|side" (def. main), servings (def. 2, ≥1)}]}` | `{created:<i64>, shopping_list:[ShoppingListGroup]}` | `agent:plan` |
| `POST /api/agent/shopping-items` | `{items:[IngredientInput]}` donde `IngredientInput = {name, amount:f64, unit, category?:Option<str>}` | `[ShoppingListGroup]` | `agent:shopping` |
| `DELETE /api/agent/shopping/completed` | — | `{removed:<u64>}` | `agent:shopping` |
| `POST /api/agent/recipes` | `RecipeInput` (idéntico a `POST /api/recipes`) | `201` + `Recipe` | `agent:write` |

`RecipeInput` = `{name, category, diet_type?, prep_time_minutes?, servings?, ingredients:[IngredientInput] (≥1), instructions?, source_url?, tags?:[str], auto_tags?:bool (def. true), suppressed_auto_tags?:[str]}`.

Categorías válidas de receta: `main|salad|side|dessert|breakfast`.

### 0.3 Autenticación (decisión del usuario: **solo OIDC M2M**)

La API key estática `OXIMEAL_API_KEY` está **obsoleta y ya no se acepta**. Con `OIDC_ENABLED=true`, la Agent API exige `Authorization: Bearer <access-token>` (RFC 9068) validado contra el `aud` = `OIDC_API_AUDIENCE`, leyendo los permisos del claim `scope`. El emisor es **PocketID** (el mismo IdP que ya usa Valet en `AUTH_ISSUER_URL`).

El camino de máquina es **`grant_type=client_credentials`** contra el `token_endpoint` del IdP, con `scope=agent:read agent:plan agent:shopping agent:write`.

> Con `OIDC_ENABLED=false` la API queda abierta, pero **por decisión del usuario** Valet implementará únicamente la vía M2M (Bearer con token obtenido por `client_credentials`). No se implementa modo abierto.

### 0.4 Patrón de integración nativa de Valet (molde a replicar)

| Pieza | Molde existente | A crear para OxiMeal |
|---|---|---|
| Cliente HTTP | `src/services/opentodo.rs`, `src/services/zennotes.rs` | `src/services/oximeal.rs` |
| Herramienta | `src/tools/zennotes.rs` (multioperación, `operation` + permiso por args) | `src/tools/oximeal.rs` |
| Rutas | `src/routes/opentodo.rs` (`status`, `check`, `token`) | `src/routes/oximeal.rs` |
| Handlers | `src/handlers/opentodo.rs` | `src/handlers/oximeal.rs` |
| Ajustes sensibles | `SENSITIVE_KEYS` en `src/db/repos/settings.rs` | `+ oximeal_client_secret` |
| Catálogo de skills | `src/orchestrator/skills.rs` (enum `Skill` + `CATALOG` + `ALL_SKILLS`) | `+ Skill::Menus` |
| Fragmento de prompt | migración `migrations/20261007000001_skill_router.sql` (valores en `settings`) | nueva migración con `SKILL_MENUS_PROMPT` |
| Registro | `build_tool_registry()` y `.merge(routes::…)` en `src/lib.rs` | registrar tool y rutas |
| UI de ajustes | `frontend/src/components/ZennotesSection.tsx`, `OpentodoSection.tsx` + `hooks/useZennotes.ts`, `useOpentodo.ts`, pestaña «Integraciones» de `SettingsDialog.tsx` | `OximealSection.tsx` + `useOximeal.ts` |

Cifras de partida: catálogo de **12 skills** y **33 herramientas** registradas (verificado por los tests de integridad del catálogo: `catalog_declares_exactly_the_twelve_wide_domain_ids`, `catalog_plus_core_covers_all_thirty_three_tools_without_orphans`). Este plan los lleva a **13 skills y 34 herramientas**.

### 0.5 Avisos / discrepancias detectadas

- **El ejemplo `curl` del README de OxiMeal está desactualizado:** envía `{"preferences":{"servings":2}}` a `POST /api/agent/weekly-plan`, pero el handler real (`StructuredPlan`) espera `{"entries":[…]}`. **Manda el código**, no el README.
- **OxiMeal NO está desplegado en `co1`** (hay un stack `mealie` y ninguno `oximeal`). Por decisión del usuario, este plan **solo implementa el lado Valet**; el despliegue de OxiMeal queda fuera de alcance.
- `reqwest 0.13` (json+rustls+stream) y `jsonwebtoken 11` ya están en el `Cargo.toml` de Valet; `wiremock 0.6` está en `dev-dependencies`. No se añaden dependencias nuevas si se evitan.

---

## 1. Objetivos

- **G1** — Valet puede **leer recetas**: consultar las planificadas recientemente y buscar por texto/categoría.
- **G2** — Valet puede **planificar el menú de la semana**: fijar el plan semanal (fecha, comida, receta, rol, raciones) y devolver la lista de la compra reconstruida.
- **G3** — Valet puede **gestionar la lista de la compra**: añadir/sumar ingredientes y purgar los ya comprados.
- **G4** — Valet puede **guardar recetas nuevas** dadas de alta desde el chat.
- **G5** — La integración **autentica con OIDC M2M** (`client_credentials` contra PocketID), con el secreto tratado como ajuste sensible y **nunca** expuesto por API, logs, `Debug` ni mensajes de error.
- **G6** — Todo se enruta por el **catálogo de skills** (nueva skill `menus`) y se configura desde el **frontend → Ajustes → Integraciones → OxiMeal**, con sonda de conexión real.

---

## 2. Decisiones de diseño (bloqueadas salvo orden explícita)

- **D1 — Una única herramienta `oximeal`, multioperación**, con parámetro `operation` (molde `zennotes`/`calendar`). No se crean 6 herramientas.
- **D2 — Skill nueva `menus`** (id `menus`, prompt `SKILL_MENUS_PROMPT`). Agrupa la herramienta `oximeal`. Umbral por defecto `0.10`, como el resto.
- **D3 — Claves de ajuste:**
  - `oximeal_base_url` — **no sensible**. Def. `https://oximeal.territoriolinux.es`.
  - `oximeal_issuer_url` — **no sensible**. Def. vacío ⇒ se cae a `AUTH_ISSUER_URL` de la config de Valet.
  - `oximeal_client_id` — **no sensible** (viaja por `PUT /api/settings`).
  - `oximeal_client_secret` — **SENSIBLE** (entra en `SENSITIVE_KEYS`): `GET /api/settings` lo omite y `PUT /api/settings` lo ignora. Su única puerta es `PUT /api/oximeal/secret`.
- **D4 — Token cacheado en memoria de proceso**, con margen de seguridad (30 s) antes de su expiración, y descubrimiento cacheado del `token_endpoint`. Se lee la configuración **en cada llamada** para que editar ajustes surta efecto sin reiniciar.
- **D5 — Endpoints de integración en Valet:** `GET /api/oximeal/status`, `GET /api/oximeal/check`, `PUT /api/oximeal/secret`.
- **D6 — `GET /api/oximeal/check` es la sonda real:** obtiene token y llama a `GET /api/agent/recent-recipes?days=1`. Responde **siempre `200`** con `{ok, detail}`; `detail` distingue «falta configuración», «el IdP no dio token», «token rechazado» y «OxiMeal no responde». **Nunca** devuelve el secreto ni el token.
- **D7 — Permisos de la herramienta por operación** (molde zennotes): lecturas → `NoConfirm`; escrituras aditivas (`add_shopping_items`, `create_recipe`) → `Notify`; mutaciones que **reemplazan o destruyen** (`plan_week` sobrescribe el plan y reconstruye la compra; `clear_purchased`) → `ExplicitApproval`. **Punto abierto para el usuario** (ver §7).
- **D8 — Errores en español y accionables**, sin filtrar credenciales. Variantes: `NotConfigured`, `Unauthorized`, `NotFound`, `InvalidInput(String)`, `Upstream(String)`.
- **D9 — Sin despliegue de OxiMeal en este plan.** Sin cambios en `docker-compose*.yml`. Valet solo apunta a `oximeal_base_url`.
- **D10 — SDD + TDD obligatorio:** primero el change proposal en `openspec/changes/oximeal-integration/`, aprobación del usuario, y después RED→GREEN→REFACTOR por componente.

---

## 3. Mapa arquitectónico (ficheros)

### Backend Rust (Valet)
| Fichero | Acción |
|---|---|
| `src/services/oximeal.rs` | **NUEVO** — cliente `Oximeal`, `OximealConfig` (Debug redactado), `OximealError`, resolución de config, token M2M + caché, 6 operaciones. |
| `src/services/mod.rs` | `pub mod oximeal;` |
| `src/tools/oximeal.rs` | **NUEVO** — `OximealTool`: `name/description/parameters/permission/execute` + tests. |
| `src/tools/mod.rs` | `pub mod oximeal;` |
| `src/routes/oximeal.rs` | **NUEVO** — `routes()` con los 3 endpoints. |
| `src/routes/mod.rs` | `pub mod oximeal;` |
| `src/handlers/oximeal.rs` | **NUEVO** — `status`, `check`, `update_secret` + tests. |
| `src/handlers/mod.rs` | `pub mod oximeal;` |
| `src/db/repos/settings.rs` | `+ "oximeal_client_secret"` en `SENSITIVE_KEYS` + test. |
| `src/orchestrator/skills.rs` | enum `Skill::Menus`; `CATALOG` 12→13; `ALL_SKILLS` 12→13; entrada nueva; tests de integridad actualizados. |
| `src/lib.rs` | registrar la tool en `build_tool_registry()` y `.merge(routes::oximeal::routes())`. |
| `migrations/<nuevo>_oximeal_skill.sql` | **NUEVO** — siembra `SKILL_MENUS_PROMPT` (+ umbral si aplica) en `settings`. |
| `openspec/changes/oximeal-integration/` | **NUEVO** — `proposal.md`, `tasks.md`, `specs/{oximeal-service,oximeal-api,tools,frontend}/spec.md`. |
| `.env.example` | documentar `OXIMEAL_BASE_URL`, `OXIMEAL_ISSUER_URL`, `OXIMEAL_CLIENT_ID`, `OXIMEAL_CLIENT_SECRET` (opcionales). |
| `AGENTS.md` | añadir la fila del `PLAN-021` al índice de `plans/`. |

### Frontend (Valet)
| Fichero | Acción |
|---|---|
| `frontend/src/components/OximealSection.tsx` | **NUEVO** — ajustes: base_url, issuer_url, client_id, client_secret (siempre vacío al abrir), botón «Probar conexión». |
| `frontend/src/hooks/useOximeal.ts` | **NUEVO** — `status`, `check`, `saveSecret`. |
| `frontend/src/components/SettingsDialog.tsx` | entrada en la pestaña «Integraciones». |
| `frontend/src/test/OximealSection.test.tsx`, `useOximeal.test.ts` | **NUEVOS**. |
| `frontend/src/lib/api.ts` (o equivalente) | métodos del cliente HTTP si el proyecto los centraliza. |

---

## 4. Contratos

### 4.1 Ajustes (Valet)

| Clave | Sensible | Defecto |
|---|---|---|
| `oximeal_base_url` | No | `https://oximeal.territoriolinux.es` |
| `oximeal_issuer_url` | No | `""` ⇒ fallback a `AUTH_ISSUER_URL` |
| `oximeal_client_id` | No | `""` |
| `oximeal_client_secret` | **Sí** | `""` |

### 4.2 API de integración de Valet

- `GET /api/oximeal/status` → `{ configured: bool, base_url: string, issuer_url: string, has_client_id: bool }`. **Nunca** el secreto.
- `GET /api/oximeal/check` → `{ ok: bool, detail: string }` (siempre `200`). Detalles: «configuración incompleta», «el IdP no entregó token», «el token fue rechazado», «OxiMeal no responde», «conexión correcta».
- `PUT /api/oximeal/secret` con `{ "client_secret": "<str>" }` → `400` si viene vacío/ausente; `204` si se guarda (recortado). **Nunca** lo devuelve.

### 4.3 Herramienta `oximeal` (contrato del modelo)

`operation` ∈ {`recent_recipes`, `search_recipes`, `plan_week`, `add_shopping_items`, `clear_purchased`, `create_recipe`}, con `parameters` declarando:

```json
{
  "type": "object",
  "properties": {
    "operation": { "type": "string", "enum": ["recent_recipes","search_recipes","plan_week","add_shopping_items","clear_purchased","create_recipe"] },
    "days": { "type": "integer", "minimum": 1, "maximum": 365 },
    "query": { "type": "string" },
    "category": { "type": "string", "enum": ["main","salad","side","dessert","breakfast"] },
    "entries": { "type": "array", "items": { "type": "object", "properties": { "date": {"type":"string"}, "meal_type": {"type":"string","enum":["lunch","dinner","breakfast","snack"]}, "recipe_id": {"type":"string"}, "role": {"type":"string","enum":["main","salad","side"]}, "servings": {"type":"integer","minimum":1} }, "required": ["date","meal_type","recipe_id"] } },
    "items": { "type": "array", "items": { "type": "object", "properties": { "name": {"type":"string"}, "amount": {"type":"number"}, "unit": {"type":"string"}, "category": {"type":"string"} }, "required": ["name","amount","unit"] } },
    "recipe": { "type": "object", "description": "RecipeInput de OxiMeal (name, category, ingredients obligatorios)" }
  },
  "required": ["operation"]
}
```

Validación **sin salir a la red** (molde zennotes): `date` = `YYYY-MM-DD`, `meal_type`/`role`/`category` dentro de los enums, `servings ≥ 1`, `name` no vacío, `amount ≥ 0`, `entries`/`items`/`ingredients` no vacíos. Todo lo inválido → `ToolError::InvalidArguments`.

### 4.4 Token M2M (secuencia)

1. `issuer = oximeal_issuer_url` o `AUTH_ISSUER_URL`.
2. `GET <issuer>/.well-known/openid-configuration` → `token_endpoint` (cacheado).
3. `POST <token_endpoint>` con `grant_type=client_credentials`, `client_id`, `client_secret`, `scope=agent:read agent:plan agent:shopping agent:write` (`Content-Type: application/x-www-form-urlencoded`).
4. Guardar `{access_token, expires_in}` en caché; reusar mientras no falten <30 s.
5. Cada llamada a `/api/agent/*` lleva `Authorization: Bearer <access_token>`.
6. `401/403` ⇒ invalidar caché y devolver `Unauthorized` (un reintento único permitido; si vuelve a fallar, error).

> **Prerrequisito de despliegue (documentar, no ejecutar):** en PocketID hay que crear un **cliente M2M** con acceso al API de OxiMeal y los permisos `agent:read`, `agent:plan`, `agent:shopping`, `agent:write`, y copiar su `client_id`/`client_secret` en Valet.

---

## 5. Tareas (TDD + SDD)

### Fase 0 — SDD (spec primero, SIN código)
- [ ] T0.1 `openspec new change oximeal-integration`.
- [ ] T0.2 `proposal.md` con el *Why* (Agent API existente + decisión M2M) y el *What*.
- [ ] T0.3 Deltas de spec con `GIVEN/WHEN/THEN`:
  - `specs/oximeal-service/spec.md` — resolución de config, token M2M + caché, redacción de credenciales, traducción de errores, 6 operaciones.
  - `specs/oximeal-api/spec.md` — los 3 endpoints, contrato exacto y el invariante «nunca se expone el secreto».
  - `specs/tools/spec.md` — la herramienta `oximeal`, sus operaciones, validación y permisos; y la skill `menus` en el catálogo (13 skills / 34 tools).
  - `specs/frontend/spec.md` — sección de ajustes «OxiMeal» en «Integraciones».
- [ ] T0.4 `tasks.md` con el checklist RED/GREEN/REFACTOR.
- [ ] T0.5 `openspec validate oximeal-integration --strict`.
- [ ] T0.6 **STOP: presentar al usuario y esperar aprobación.**

### Fase 1 — Servicio `oximeal` (RED→GREEN→REFACTOR)
- [ ] T1.1 RED — tests `wiremock` en `src/services/oximeal.rs`: descubrimiento de `token_endpoint`, petición `client_credentials` con el `scope` correcto, caché (no repite la petición antes de expirar), re-POST tras expirar, `401`→`Unauthorized`, respuesta `5xx`→`Upstream`, `Debug` de `OximealConfig` **no** contiene el secreto, `NotConfigured` sin credenciales. `cargo test` → RED.
- [ ] T1.2 GREEN — implementar `Oximeal` + `OximealConfig` + `OximealError` + `resolve_config` (ajustes → entorno → defecto solo para `base_url`) + `from_settings`.
- [ ] T1.3 RED — un test por operación (`recent_recipes`, `search_recipes`, `set_weekly_plan`, `add_shopping_items`, `delete_completed`, `create_recipe`) con cuerpos/consultas correctos y mapeo de respuesta.
- [ ] T1.4 GREEN — implementar las 6 operaciones.
- [ ] T1.5 REFACTOR — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

### Fase 2 — Herramienta `oximeal`
- [ ] T2.1 RED — tests de `parameters()` (esquema válido), validación de args por operación (rechazo sin red), `permission()` por operación (D7), traducción de errores a mensajes accionables, y que `name()=="oximeal"`.
- [ ] T2.2 GREEN — implementar `OximealTool` con despacho por `operation`.
- [ ] T2.3 RED/GREEN — registrar en `build_tool_registry()` de `src/lib.rs`; el test de integridad del catálogo **debe fallar** hasta añadir la skill (ver Fase 4).
- [ ] T2.4 REFACTOR — fmt/clippy/test.

### Fase 3 — Rutas y handlers de integración
- [ ] T3.1 RED — tests de `status` (sin secreto ni token; `configured` fiel), `check` (los 4 detalles + «conexión correcta» con wiremock), `update_secret` (`400` vacío/ausente, `204` guardado recortado). Molde `src/handlers/opentodo.rs`.
- [ ] T3.2 GREEN — `src/handlers/oximeal.rs` + `src/routes/oximeal.rs`; registrar `.merge(routes::oximeal::routes())` en `src/lib.rs`.
- [ ] T3.3 `src/db/repos/settings.rs`: añadir `oximeal_client_secret` a `SENSITIVE_KEYS` + test de que `GET /api/settings` lo omite.
- [ ] T3.4 REFACTOR — fmt/clippy/test.

### Fase 4 — Catálogo de skills y migración
- [ ] T4.1 RED — actualizar los tests de integridad del catálogo a 13 skills / 34 tools y añadir `Skill::Menus` a `ALL_SKILLS`. `cargo test` → RED.
- [ ] T4.2 GREEN — añadir `Skill::Menus` al enum, la entrada al `CATALOG` (`tools: &["oximeal"]`, `prompt_key: "SKILL_MENUS_PROMPT"`, `threshold: 0.10`) y `pub mod oximeal` en `src/tools/mod.rs`.
- [ ] T4.3 Migración nueva con el texto por defecto de `SKILL_MENUS_PROMPT` (guía: cuándo usar OxiMeal; que `plan_week` exige `recipe_id` obtenido antes con `search_recipes`; que `add_shopping_items` suma a la lista).
- [ ] T4.4 REFACTOR — fmt/clippy/test; `openspec validate`.
- [ ] T4.5 **(medición, opcional, fuera del bloqueo)** evaluar el enrutado de `menus` con el arnés `valet-route-eval` sobre turnos sintéticos, como se hizo con `zennotes`. Sin tocar el umbral salvo evidencia.

### Fase 5 — Frontend
- [ ] T5.1 RED — `useOximeal.test.ts` (estados `loading/configured/degraded`, `check`, no releer el secreto) y `OximealSection.test.tsx` (campos, «Probar conexión», el secreto arranca vacío).
- [ ] T5.2 GREEN — `useOximeal.ts` + `OximealSection.tsx` + entrada en la pestaña «Integraciones» de `SettingsDialog.tsx`.
- [ ] T5.3 REFACTOR — `npx tsc --noEmit`, `npm run lint`, `npx vitest run`. **Vigilar el límite de bundle inicial** (69,99 kB / 70,00 kB): usar `lazy()` si el peso lo exige.

### Fase 6 — Documentación y cierre
- [ ] T6.1 `.env.example`: documentar las 4 variables `OXIMEAL_*` (opcionales) y el prerrequisito del cliente M2M en PocketID.
- [ ] T6.2 `AGENTS.md`: fila del `PLAN-021` en el índice de `plans/`.
- [ ] T6.3 Puertas completas: `just check-all` (fmt + clippy + `cargo test` + frontend lint/test/check) y `openspec validate --all --strict`.
- [ ] T6.4 Auditoría: `@rust-reviewer` sobre servicio/tool/rutas y `@react-reviewer` sobre la UI de ajustes; enrutar hallazgos a los devs.
- [ ] T6.5 `openspec archive oximeal-integration` (respetando la regla de headers exactos de la spec destino).
- [ ] T6.6 Gitflow: ramas `feature/oximeal-integration`, commits gitmoji, PR a `development`.

---

## 6. Verificación final (end-to-end)

- [ ] V1 — `cargo test` (lib + integración) 100 % verde; `cargo clippy --all-targets -- -D warnings` sin avisos; `cargo fmt --check` limpio.
- [ ] V2 — `npx vitest run`, `npx tsc --noEmit`, `npm run lint` verdes.
- [ ] V3 — `openspec validate --all --strict` verde.
- [ ] V4 — Con un OxiMeal accesible **y** un cliente M2M válido: `GET /api/oximeal/check` → `{ok:true}`; `search_recipes` devuelve recetas; `plan_week` fija el plan y devuelve la `shopping_list`; `add_shopping_items` suma; `clear_purchased` purga; `create_recipe` da de alta y luego aparece en `search_recipes`. **Esta verificación requiere el despliegue de OxiMeal + cliente M2M, que está fuera de alcance; si no se dispone de ellos, se documenta como pendiente y se cierra el resto.**
- [ ] V5 — `GET /api/settings` **no** contiene `oximeal_client_secret`; ningún log/`Debug` de la aplicación lo contiene.
- [ ] V6 — Desde el chat, un turno «prepárame el menú de la semana» activa la skill `menus` y expone la herramienta `oximeal`.

---

## 7. Puntos abiertos para el usuario

1. **D7 — permisos por operación.** Propuesta: `plan_week` y `clear_purchased` con `ExplicitApproval`; `add_shopping_items` y `create_recipe` con `Notify`; lecturas `NoConfirm`. ¿Se confirma?
2. **`oximeal_client_id` no sensible.** Propuesta: viaja por `PUT /api/settings`. Alternativa: tratarlo también como sensible (endpoint dedicado recibiendo `client_id` + `client_secret`). ¿Preferencia?
3. **Nombre de la skill.** Propuesta: `menus`. Alternativas: `cocina`, `comidas`, `oximeal`. ¿Preferencia?

---

## 8. Fuera de alcance

- Despliegue/ajuste de OxiMeal (imagen, stack Dockge, Traefik) y creación del cliente M2M en PocketID.
- Modo abierto (`OIDC_ENABLED=false`) en el cliente de Valet.
- Sincronización bidireccional ni caché local de recetas/plan/compra en la base de Valet.
- Cambios en `docker-compose.yml` / `docker-compose.prod.yml`.
- Importación de recetas desde URL, gestión de etiquetas y pasillos más allá de lo que expone la Agent API.
- Backfill o migración de datos en OxiMeal.

---

## 9. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| PocketID no acepta `client_credentials` sin `resource`/audiencia explícita | La sonda `/check` distingue «el IdP no entregó token» y el plan acepta añadir el parámetro `resource` si la verificación lo exige. |
| OxiMeal no desplegado aún | Alcance limitado a Valet; la verificación E2E se marca pendiente y el resto de puertas cierran. |
| Crecimiento del bundle inicial al añadir la sección de ajustes | `lazy()`/code-splitting; el límite no se sube. |
| Tokens M2M de vida corta y concurrencia | Caché única en proceso + `tokio::sync::Mutex`; un solo refresco. |
| Fuga del secreto por logs/errores | `Debug` redactado, `SENSITIVE_KEYS`, tests que afirman la ausencia del secreto en la respuesta y en el `Debug`. |

---

*Fin del PLAN-021.*
