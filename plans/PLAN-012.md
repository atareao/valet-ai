# PLAN-012 — Los widgets, siempre presentes

**Estado:** activo
**Fecha:** 2026-11-01
**Origen:** queja del usuario — «Quiero que los widgets estuvieran siempre presentes. En las últimas
ocasiones me ofrece varias alternativas, y en lugar de mostrar un widget me dice que elija A, B o C…
y esto como experiencia de usuario es muy frustrante».
**Change de OpenSpec:** `openspec/changes/widgets-always-on/` (aprobado el 1-nov-2026; implementado, en
verificación sobre el despliegue).

---

## 1. Objetivo

Que `render_widget` esté **siempre presente** en el turno —herramienta expuesta y guía en el mensaje
de sistema— en lugar de depender de lo que decida el enrutador, y que la guía sea lo bastante
prescriptiva para que el modelo la invoque cuando el turno pide una elección, varios datos, un plan o
un punto en el mapa.

## 2. Diagnóstico (medido en producción, `co1`, 1-nov-2026)

| Hecho | Dato |
|:---|:---|
| Mensajes de asistente / con widget (histórico) | 466 / 16 (3,4 %) |
| Turnos y widgets del 10 al 25 de octubre | 315 turnos, 16 widgets (5,08 %) |
| Turnos y widgets del 26 de octubre a hoy | **118 turnos, 0 widgets** |
| Probabilidad de encadenar 118 turnos a cero con la tasa anterior | ≈ 0,2 % (degradación real) |
| Disparador de los 16 widgets históricos | casi siempre petición explícita («un planito?», «donde está el parking?»); el último, el usuario escribiendo `Widget` |
| Turnos de hoy con `widgets` seleccionada | 4 de 7 (57 %) |
| Invocaciones de `render_widget` en esos 4 turnos | **0** |
| Filas de `tool_results` con `render_widget` en toda la BD | **0** |
| `system_prompt` de producción (4.687 car.) | no menciona `render_widget`, `QuickForm`, `Checklist` ni `LocationWidget` |
| `SKILL_WIDGETS_PROMPT` de producción | 1.391 car., byte a byte el sembrado por `20261008000001` |
| Umbral de `widgets` | `0.20`, el único del catálogo por encima de `0.10` |
| Probabilidades de `widgets` medidas hoy | `0.10, 0.10, 0.21, 0.26, 0.29, 0.37` — apiñadas alrededor del umbral |

**Causas, por orden de peso:**

1. **El modelo no llama a la herramienta aunque la tenga delante** (0 de 4 turnos expuestos; 0 en
   todo el histórico). Es la causa dominante.
2. **La guía tiene una puerta de salida**: su regla 3 autoriza la prosa «si la respuesta se resuelve
   con una explicación conceptual o un dato directo». Sus disparadores son abstractos.
3. **Fricción alta**: la guía no nombra ni uno de los tres widgets; el modelo debe deducir el enum y
   un `data` cuyo esquema solo vive en 600 caracteres de JSON Schema.
4. **Nada refuerza los widgets en el prompt base**: la «REGLA DE ORO» habla solo de acciones, «Modo
   Conciso» pide brevedad y ninguno de los tres ejemplos del prompt es de widgets.
5. **La decisión se toma antes de que exista la respuesta**: el router clasifica el mensaje del
   usuario, no la respuesta que se está redactando; el momento en que el widget es obvio —«voy a
   escribir A/B/C»— ocurre durante la generación y nadie lo consulta.

## 3. Alcance decidido (opción E+C)

- **E — La skill `widgets` deja de estar condicionada por el clasificador.** Umbral compilado `0` y
  regla general: **umbral efectivo cero = skill no condicionada** (seleccionada en todos los turnos,
  incluso sin respuesta del clasificador). Se conserva la palanca: subir el umbral vuelve a
  condicionarla.
- **C — La guía se reescribe en prescriptivo.** Nombra los tres widgets con un ejemplo mínimo de
  `data`, ancla el disparador en comportamiento observable («si vas a escribir dos o más opciones
  etiquetadas A/B/C, no las escribas: renderiza un widget») y **retira la puerta de salida**.

**Fuera de alcance:** el esquema de `render_widget`, el frontend, la persistencia y el evento SSE.
La detección determinista de listas de opciones en prosa queda como red de seguridad de otra fase
(solo si E+C no basta).

## 4. Mapa de la arquitectura tocada

| Pieza | Fichero | Qué cambia |
|:---|:---|:---|
| Umbral compilado de `widgets` | `src/orchestrator/skills.rs` | `threshold: 0.20` → `0.0`; el test que lo fija |
| Selección con umbral cero | `src/orchestrator/skill_router.rs` | `select`: una skill a umbral `0` se selecciona aunque falte su respuesta |
| Definición de skill activa | `src/orchestrator/skill_router.rs` | `is_active` / `active_skills`: activa si está habilitada **y** (está en la selección **o** tiene umbral efectivo `0`). Definición **única**, compartida por la composición y la lectura de fragmentos |
| Lectura de fragmentos | `src/orchestrator/agent.rs` | el sitio de lectura pasa el **conjunto activo**, no `selection.skills` (que va vacía al fallar abierto) |
| Migración | `migrations/20261101000001_widgets_always_on.sql` | borra el umbral sembrado; reescribe la guía si sigue prístina |
| Spec | `openspec/specs/orchestrator/skill-router/spec.md` y `openspec/specs/db/schema/spec.md` | 5 requisitos modificados en `skill-router` y 1 en `db/schema` |

**No se toca:** `CORE_TOOLS` (un requisito prohíbe que `render_widget` esté en el core),
`src/tools/widget.rs`, `src/handlers/skills.rs`, el frontend.

**Solo spec, cero código:** `openspec/specs/db/schema/spec.md` arrastra una deuda del change archivado
`2026-10-08-skill-router-tuning`, que movió la guía fuera de `system_prompt` sin actualizar ese módulo:
su requisito sigue afirmando que una instalación nueva deja la sección en el prompt base, cuando el
`system_prompt` de producción (4.687 caracteres) no la menciona. Se corrige en este mismo change porque
es el que redefine el contenido de esa misma guía.

## 5. Tareas

El detalle vive en `openspec/changes/widgets-always-on/tasks.md`. Resumen por fases:

1. **Planificación (SDD)** — proposal, design, delta de spec, validación y aprobación del usuario.
2. **RED (Rust)** — 7 tests nuevos en el router más la corrección de los **tres** tests del catálogo
   que el umbral `0` invalida (uno de ellos, `widgets_threshold_is_strictly_above_every_domain`,
   encarna el escenario `spec.md:41` que este change retira). Cubren: umbral cero selecciona, la
   ausencia de respuesta no desactiva, la skill deshabilitada no expone ni inyecta, un umbral mayor
   que cero sigue condicionando, el fallo abierto inyecta el fragmento y el override del usuario
   gana.
3. **GREEN (Rust)** — las tres líneas de `skills.rs` y `skill_router.rs`.
4. **Migración y guía** — el SQL, y el texto de la guía que la migración siembra. El texto **ya está
   redactado y aprobado por el usuario el 1-nov-2026**: vive en `design.md` → «La guía nueva (texto
   exacto)» y la tarea 4.2 lo **transcribe carácter a carácter**, no lo reescribe.
5. **REFACTOR** — clippy, fmt, `just check-all`.
6. **Verificación** — `openspec validate --all --strict` y la batería medida sobre el despliegue.
7. **Auditoría y corrección** — auditoría de solo lectura del change, que destapó que la inyección
   del fragmento por umbral cero no llegaba al mensaje de sistema en los caminos de fallo abierto
   (el lector filtraba por una selección vacía). Corregido unificando la definición de «skill
   activa», con tests end-to-end nuevos. Detalle: `tasks.md` 7.1–7.5 y `design.md` D7.

## 6. Criterios de aceptación

- [x] `render_widget` figura en `exposed_tools` en **todos** los turnos, también con el enrutador
      apagado o caído.
- [x] `SKILL_WIDGETS_PROMPT` viaja en el mensaje de sistema de **todos** los turnos, también en el
      fallo abierto.
- [x] Con la skill `widgets` deshabilitada no se expone la herramienta ni se inyecta la guía.
- [x] Subir `ROUTER_THRESHOLD_WIDGETS` por encima de `0` vuelve a condicionar la skill.
- [x] `openspec validate --all --strict` sin errores y `just check-all` verde.
- [ ] **Batería sobre el despliegue**: los tres casos que fallaron el 1-nov renderizan un widget
      (ver `tasks.md` 6.3). Es la verificación de la guía, que no tiene test unitario.

## 7. Riesgos

| Riesgo | Mitigación |
|:---|:---|
| La guía no basta (es un cambio de prompt, sin test unitario) | Batería medida de la tarea 6.3; si falla, abrir la fase determinista |
| La guía nueva viaja en **todos** los turnos y mide 2.704 caracteres (la anterior medía 1.391) | Se acepta: es el precio de que esté siempre presente y la palanca del usuario sigue existiendo (`ROUTER_THRESHOLD_WIDGETS`) |
| Widgets de más (riesgo contrario) | La sección «Los dos únicos casos sin widget» de la guía acota esos dos casos, y la regla de las opciones se declara prioritaria sobre ellos |
| Aviso permanente de «umbral extremo» en la pestaña de `widgets` | Se acepta como información veraz; nota específica de UI como mejora posterior |
| La guarda de la migración es byte a byte: si el texto guardado difiere en un espacio o un salto de línea, la guía vieja no se sustituye y el fallo es silencioso | Se verificó byte a byte la guía de producción antes de escribirlo; la batería 6.3 lo delataría (guía vieja ⇒ cero widgets) |
| `route-eval --threshold` repone un default a todas las skills y mediría algo distinto de lo desplegado | No usar el arnés para validar este change: la validación es la batería de producción (6.3) |

## 8. Estado y siguientes pasos

1. Validar el change y **esperar la aprobación explícita del usuario** (protocolo SDD).
2. RED → GREEN → REFACTOR con TDD, delegando la escritura en los subagentes y auditando con
   `@rust-reviewer`.
3. Release (minor: añade capacidad de comportamiento) y despliegue en `co1`.
4. Batería de la tarea 6.3 sobre el despliegue y decisión sobre la fase determinista.

## 9. Relevo para la sesión siguiente (1-nov-2026)

La sección 8 de arriba quedó obsoleta: el change ya está aprobado, implementado y auditado. El estado
vigente es este.

**Estado:** código y documentación cerrados y verdes, **sin commitear**. La verificación de la guía
—la batería de 6.3— está pendiente y es lo único que falta para dar el change por bueno.

Evidencia ya obtenida: suite completa en verde (1944 pasan, 0 fallan, 1 ignorado), `cargo clippy
--all-targets -- -D warnings` limpio, `cargo fmt --check` limpio, `just check-all` en exit 0,
`openspec validate --all --strict` en 63/63, y la auditoría con su fallo crítico corregido
(`design.md` -> D7 y la tabla de riesgos residuales).

1. **Rama y commits** (gitflow + gitmoji; nunca directamente a `main` ni a `development`):
   `git checkout development && git pull origin development` y luego
   `git checkout -b feature/widgets-always-on`. Alcance del commit, **solo lo del change**:
   `src/orchestrator/skills.rs`, `src/orchestrator/skill_router.rs`, `src/orchestrator/agent.rs`,
   `tests/api/tools.rs`, `tests/db/migrations.rs`, `migrations/20261101000001_widgets_always_on.sql`,
   `openspec/changes/widgets-always-on/` y `plans/PLAN-012.md`. **Quedan fuera** (decisión del
   usuario): `AGENTS.md` —modificado antes de este trabajo—, `PLAN.md` de la raíz, y
   `docs/decisiones-cerradas.md`, `docs/entorno-produccion.md`, `docs/plans-indice.md`. El grueso es
   un `feat` (minor) del orquestador; conviene separar la migración y los tests.
2. **Release y despliegue**: release minor (añade comportamiento) -> imagen a GHCR -> despliegue en
   `co1` por Dockge.
3. **Batería de la tarea 6.3** —es el único test de la guía, que no tiene test unitario—. Con el
   release desplegado, repetir los tres prompts reales del 1-nov, que se respondieron en prosa A/B/C:
   «que mas tenemos que despachar?», «donde utilizo nicegui?» y «ghostty versión 1.4 no está para
   Arch?». Criterio: **los tres renderizan un widget**.
4. **Comprobación en la BD de producción** (`tasks.md` 6.4): que `messages.widgets` deja de estar
   vacío y que `tools_used` registra `render_widget`. En `co1` no hay `sqlite3`: usar `sudo python3`
   con `sqlite3.connect("file:...?mode=ro", uri=True)` más `backup` —la BD está en WAL; nunca
   `immutable=1`—. Volumen: `valet_valet_data` ->
   `/var/lib/docker/volumes/valet_valet_data/_data/valet.db`.
5. **Si la batería falla**: la guía no basta y se abre la fase determinista (detectar listas de
   opciones en la respuesta y renderizar un widget), hoy fuera de alcance.
6. **Archive** (`tasks.md` 6.5): `openspec archive widgets-always-on`, solo con la batería aprobada.

**Nota de higiene:** no usar `route-eval --threshold` para validar este change (riesgo 5 de la
sección 7). **Pendiente diferido, ajeno a este plan:** la comprobación visual de las pestañas
verticales de ajustes al desplegar.
