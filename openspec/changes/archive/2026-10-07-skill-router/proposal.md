# Proposal

## Why

Cada turno envía al LLM las **13 definiciones de herramientas** completas y las **reenvía en cada iteración** del bucle ReAct (`src/orchestrator/agent.rs:737`). Medido sobre los `parameters()` de las tools reales, los esquemas suman **~11,8 KB** y, con descripciones y envoltorio `{"type":"function",…}`, **~14 KB ≈ 3,5-4 k tokens por petición**; con 2-3 iteraciones por turno, entre 7 y 12 k tokens de esquemas por turno. El prompt caching amortigua el coste en dólares, pero no la **latencia** ni el **ruido**: trece herramientas compitiendo por la misma decisión es causa directa de elección equivocada y de parámetros mal formados.

La oportunidad es decidir **antes** del modelo principal qué habilidades necesita el turno y exponerle solo esas herramientas, dejando la decisión en un modelo de decisión tipado (Jev, TypeSafe, vía OpenRouter: `POST /api/alpha/decisions`, primitiva `noul`, una llamada por turno, ~$0,00002).

## What Changes

- Nuevo **catálogo cerrado de skills** en código: cada skill agrupa las tools de un dominio **con sus prerrequisitos** (`clima` incluye `geocode`; `lugares` incluye `geocode` y `reverse_geocode`), y hay un **conjunto core** no enrutable (`render_widget`, `get_current_time`) que se expone siempre.
- Nuevo **`SkillRouter`**: **una sola decisión por turno**, antes del bucle ReAct, con el mensaje actual y los últimos turnos; **una pregunta `noul` por skill enrutable cuyas tools estén habilitadas**, todas en una única petición HTTP.
- La petición al LLM pasa a llevar **(core ∪ skills seleccionadas) ∩ habilitadas**. La selección decide también el turno **sin herramientas** (todas las probabilidades por debajo del umbral → solo el core).
- **Fallo abierto** como propiedad de la feature: apagada, sin API key, sin skills enrutables, error, timeout, respuesta ilegible o ids desconocidos → **todas las herramientas habilitadas** y prompt intacto.
- Nuevos **fragmentos de prompt por skill** (`settings.SKILL_<ID>_PROMPT`): se inyectan en el mensaje de sistema **solo para las skills activas**, se omiten si están vacíos y **no se duplican** si el prompt base ya contiene su encabezado. El prompt de personalidad no se toca.
- **Configuración en `settings` con hot-reload** (patrón `GENERATION_*`), **apagada por defecto**: `ROUTER_ENABLED=false`, `ROUTER_MODEL=typesafe/jev-1.13`, `ROUTER_THRESHOLD=0.3`, `ROUTER_TIMEOUT_MS`, `ROUTER_HISTORY_TURNS`, sembradas por migración idempotente que no sobrescribe valores existentes. Modelo, umbral e interruptor editables desde la UI; endpoint y API key por entorno.
- Nuevo **cliente de la Decisions API de Jev** detrás de un trait (`DecisionsProvider`), aislado del endpoint `alpha` y con el modelo **pineado**.
- Nueva **UI**: interruptor, umbral, modelo y mapa de skills en la pestaña «Herramientas»; fragmentos editables en la pestaña «Prompts».
- Nuevo **arnés de evaluación** sobre el historial real (`messages.tools_used` como verdad de referencia) que mide la **cobertura de las herramientas realmente usadas** sin necesidad de etiquetar nada: es el criterio para fijar el umbral antes de encender.
- **Sin cambios** en permisos, guardrails, contrato SSE ni flujo OIDC.

## Capabilities

### New Capabilities
- `orchestrator/skill-router`: catálogo de skills, decisión de enrutado por turno, ensamblado de los fragmentos de prompt, política de fallo abierto, configuración en `settings` y arnés de evaluación.
- `llm/decisions`: cliente de la Decisions API de Jev (contrato de petición, tipado de las respuestas y errores).
- `skill-router-ui`: interruptor, umbral, modelo y mapa de skills en «Herramientas», y edición de los fragmentos de prompt en «Prompts».

### Modified Capabilities
- `orchestrator/agent`: se añade el requisito "El orquestador SHALL enrutar las herramientas por turno antes del bucle ReAct", que fija que la petición lleve el subconjunto decidido y que el fallo abierto conserve el comportamiento actual.
- `tools/registry`: se añade el requisito "El registry SHALL poder devolver las definiciones de un subconjunto", que fija el filtrado por nombre, la exclusión de deshabilitadas y el orden determinista.

## Impact

- `src/orchestrator/`: nuevo `skills.rs` (catálogo), nuevo `skill_router.rs` (decisión + ensamblado de fragmentos) y `agent.rs` (el bucle usa la selección; hoy `agent.rs:737`).
- `src/llm/`: nuevo `decisions.rs` (trait `DecisionsProvider` + cliente Jev). `src/tools/registry.rs`: `definitions_for`.
- Nueva migración de `settings` (`ROUTER_*`, `SKILL_<ID>_PROMPT`); lectura en caliente.
- Nuevo bin `src/bin/route-eval.rs` (arnés de evaluación), en la línea de `seed` y `valkey-reindex`.
- `frontend/src/components/SettingsDialog.tsx`: pestaña «Herramientas» y pestaña «Prompts».
- Sin cambios en `docker-compose*.yml`, en el contrato SSE ni en el flujo OIDC. `OPENROUTER_API_KEY` se reutiliza; se añade `ROUTER_BASE_URL` (por defecto `https://openrouter.ai/api`).
