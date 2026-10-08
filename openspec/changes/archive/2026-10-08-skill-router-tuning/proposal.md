# Proposal

## Why

La primera medición del arnés dejó el enrutador **apagado**: 59,1 % de cobertura con el umbral por defecto. Una campaña de medidas sobre los **66 turnos reales** de la base de datos (6 ejecuciones, ~$0,02) identificó las causas y el diseño que las resuelve:

1. **El umbral 0,3 era el culpable principal**, no las `criteria`: a 0,05-0,10 la cobertura sube a 92-98 %.
2. **El catálogo de 8 skills separaba dominios en distinciones finas** (`tasks` vs `reminders`, `notes` vs `unified_search`) que el modelo principal resuelve mejor con ambas herramientas a la vista. El catálogo grueso domina al fino en todo el rango de umbrales y con **menos preguntas**.
3. **Los criterios en inglés rinden peor** que en español en casi todo el rango: la hipótesis de que Jev no discriminaba en castellano queda **refutada por datos**.
4. **Los criterios compactos dominan a los verbosos** a igual cobertura (4,26 activaciones por turno frente a 4,80): las cláusulas extra y los ejemplos vuelven las preguntas sobre-inclusivas.
5. **`render_widget` —28 % del bloque— es discrecional**: su probabilidad base es alta (p50 0,21) y necesita umbral propio. Enrutarlo es la mayor palanca individual: sin él y sin enrutar `agenda`, el ahorro se quedaba en ~16 %; con ambos llega al 28,1 %.

El mejor punto medido da **97,0 % de cobertura —el techo de la métrica: los dos fallos son turnos donde el modelo usó herramientas por iniciativa propia ante un saludo— con 28,1 % de ahorro del bloque de herramientas** (10,15 herramientas por turno en vez de 13), a ~300 ms y ~$0,00005 por turno.

El obstáculo para seguir mejorando es que **las `criteria` son código compilado**: tunearlas exige recompilar y recurrir a scripts desechables, y esa fricción es la que ha hecho de este ajuste una campaña de medidas manual.

## What Changes

- **Catálogo de 8 skills a 6 de dominio amplio**: `agenda` (calendar), `pendientes` (tasks + reminders), `recuerdos` (notes + unified_search), `entorno` (weather + geocode + reverse_geocode + search_places), `web` (web_search) y `widgets` (render_widget).
- **Core reducido** a `get_current_time` y `get_current_location`; `render_widget` deja de estar siempre expuesto.
- **`criteria` compactas en español**, y **editables desde la UI guardándose en `settings`** (`SKILL_<ID>_QUESTION`, `SKILL_<ID>_CRITERIA_TRUE`, `SKILL_<ID>_CRITERIA_FALSE`), con **el valor compilado como default y fallback**: una fila ausente o vacía nunca envía un criterio vacío ni degrada el enrutado en silencio. Se leen en cada turno.
- **Umbral por skill**: `ROUTER_THRESHOLD` pasa de 0,3 a **0,10** y se añade el override `ROUTER_THRESHOLD_<ID>` (sembrado para `widgets` a **0,20**). `ROUTER_HISTORY_TURNS` pasa de 2 a **6** (la medición se hizo con seis turnos de historial y las continuaciones eran la clase de fallo dominante).
- **La guía de widgets se muda** del prompt base a `SKILL_WIDGETS_PROMPT`: ya no puede residir permanentemente en un prompt que **ordena** usar una herramienta que deja de estar siempre disponible.
- El **arnés** mide con los valores **efectivos** de `settings` y **publica la configuración con la que ha medido** (skills, umbrales y si los criterios están sobrescritos), de modo que cada ejecución sea atribuible y el bucle de ajuste pase a ser «editar en la UI → medir».
- **`/api/skills`** expone el valor efectivo de cada criterio y umbral y si está sobrescrito.
- **UI**: la sub-pestaña «Skills» agrupa por skill la pregunta, los dos criterios y el fragmento —listadas **desde el catálogo**, no por patrón de clave, para no mostrar fragmentos huérfanos—; el control del enrutador muestra el umbral efectivo de cada skill.
- **Sin cambios** en: las herramientas (esquemas, operaciones, permisos), el contrato SSE, el flujo OIDC y la capa de guardrails.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/skill-router`: el catálogo pasa a seis dominios amplios con el core reducido y `render_widget` enrutado; las `criteria` pasan a ser editables en `settings` con el valor compilado como fallback; se añade el umbral por skill y se sube `ROUTER_HISTORY_TURNS`; la guía de widgets se muda al fragmento de su skill; el arnés publica la configuración efectiva con la que mide.
- `skill-router-ui`: el control del enrutador muestra el umbral efectivo por skill, y la sub-pestaña de skills pasa a editar pregunta, criterios y fragmento por skill, listando las skills desde el catálogo.

## Impact

- `src/orchestrator/skills.rs` (catálogo de seis dominios, criterios y umbrales por defecto), `src/orchestrator/skill_router.rs` (criterios y umbrales efectivos leídos de `settings`).
- `src/handlers/skills.rs` y la ruta `/api/skills` (valores efectivos y marca de sobrescrito).
- Nueva migración de `settings`: `ROUTER_THRESHOLD` (0,10), `ROUTER_THRESHOLD_WIDGETS` (0,20), `ROUTER_HISTORY_TURNS` (6), y la **retirada del bloque de widgets** de `settings.system_prompt` hacia `SKILL_WIDGETS_PROMPT`.
- `src/bin/route-eval.rs` (configuración efectiva en el informe).
- `frontend/src/components/RouterControl.tsx`, `SkillPromptFields.tsx`, `SettingsDialog.tsx`, `src/api/client.ts`, `src/types`.
- **Medición de referencia**: 66 turnos reales, 6 ejecuciones del experimento, ~$0,02. El enrutador **sigue apagado** por defecto; el encendido es una decisión posterior con estos números delante.
