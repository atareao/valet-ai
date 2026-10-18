# Design: La bitácora como registro

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño de **este** trozo:

- `plans/PLAN-007.md` decide la forma de la bitácora y parte el trabajo en dos changes
  (`journal-entries`, la mitad registrable, y `journal-worker`, la autonomía). Aquí solo se
  documenta e implementa el primero: la autonomía necesitará después la tabla y el repositorio.
- El repositorio ya tiene el molde que se imita en cada capa: una migración idempotente por feature
  (`migrations/20261012000002_notification_ingest.sql`), un repositorio por tabla en
  `src/db/repos/`, una herramienta por dominio en `src/tools/`, rutas finas que delegan en
  `handlers` y una vista diferida por sección en el front (`frontend/notifications`).
- El enrutador de skills agrupa las herramientas por dominio **incluyendo prerrequisitos**
  (`src/orchestrator/skills.rs`) y un test de integridad falla si una herramienta registrada no
  pertenece al core ni a ninguna skill.
- La sesión OIDC la impone el middleware sobre `/api/*`; las rutas nuevas no la comprueban a mano.
- `profiles` se resuelve con `ProfilesRepo::get_or_create`, como en el resto de la API de sesión.

## Goals / Non-Goals

**Goals:**

- Un almacén propio donde una entrada por perfil y día es la unidad, con la fecha como clave de
  lectura y escritura.
- Que la bitácora se lea y se escriba igual desde la app y desde el agente, con la misma semántica
  (upsert del día) y sin duplicar filas.
- Que borrar sea un acto deliberado y consciente (es el diario del usuario).
- Que la entrada nueva sea **relato** con estructura opcional, no un recuento de hechos (eso ya es
  el timeline).

**Non-Goals:**

- No se escribe nada solo: sin worker, sin prompt, sin ajustes, sin marca de día juzgado. Eso es
  `journal-worker` (ver `proposal.md` — Non-goals).
- No se unifica la bitácora con el timeline ni con `notes`: son almacenes distintos y este change no
  los conecta.
- No se toca `docker-compose.prod.yml`.

## Decisions

**D1 — Tabla propia `journal_entries`, no notas con categoría `journal`.**
Hace falta una entrada por día, un ánimo con valores cerrados y campos con estructura (logros,
dificultades, etiquetas) que después permitan leer tendencias. En `notes` la categoría `journal` ya
existe, pero es texto libre sin fecha única, sin ánimo y sin estructura: forzarla obligaría a
parsear contenido. Es la decisión D9 de `plans/PLAN-007.md`. La tabla se crea con una migración
idempotente y aditiva, en el estilo de las existentes (`CREATE TABLE IF NOT EXISTS` + índices
`IF NOT EXISTS`), y **sin `CHECK` de ánimo**: el conjunto cerrado se garantiza en el modelo
(ver D3), porque añadir un valor futuro no debe exigir una reconstrucción de tabla.

**D2 — La fecha es la clave pública; el `id` es interno.**
Un diario se lee y se escribe **por día**. `UNIQUE(profile_id, date)` es lo que hace posible el
upsert del día: la segunda escritura sobre la misma fecha reemplaza la fila en vez de crear otra.
El repositorio se direcciona por `(profile_id, date)` en `get`, `upsert` y `delete`; el `id`
(UUID) se conserva para referencias internas pero no es la puerta de entrada. La fecha se guarda
ISO `YYYY-MM-DD` **en la zona del usuario** (decisión D2 de `PLAN-007.md`); el cambio UTC↔local es
responsabilidad de quien escribe (el worker, en el change siguiente), no del repositorio. El índice
de listado se define sobre `(profile_id, date DESC)` para servir el orden «más reciente primero».

**D3 — El ánimo es estricto y puede faltar.**
`enum Mood { Great, Good, Neutral, Bad, Terrible }` con serialización en `snake_case`
(`great`/`good`/`neutral`/`bad`/`terrible`). Un valor fuera del conjunto **NO** es válido: no se
inventa un ánimo ni se traga el dato, se rechaza (`InvalidArguments` en la herramienta, `400` en la
ruta, error de deserialización en el repositorio). Ausente (`None`/`NULL`) SÍ es válido: hay días
que se cuentan sin etiquetar el ánimo, y la entrada se guarda igual. Mismo criterio para los tres
campos JSON (`achievements`, `struggles`, `tags`): `NULL` en la base de datos se expone como lista
vacía `[]` en el modelo, y una lista vacía se guarda como `[]` o `NULL` sin ambigüedad de lectura.

**D4 — Borrar exige confirmación; leer y escribir, no.**
La herramienta `journal` declara `Permission::NoConfirm` para `get_entry`, `write_entry`,
`update_entry` y `list_entries` —el usuario ha pedido apuntar, corregir o leer— y
`Permission::ExplicitApproval` para `delete_entry`, en línea con el requisito general del registry
que ya hace destructivas las operaciones de borrado (`tasks.delete_task`, `timeline_delete_event`) y
con el hecho de que la bitácora es el diario del usuario. `write_entry` sobrescribe la entrada del
día, pero no es destructivo en el sentido del registry: es el «apúntame esto» que el usuario ha
pedido, y la entrada anterior queda versionada por `created_at`/`updated_at` en la misma fila.

**D5 — La herramienta entra en la skill existente de notas (`recuerdos`), sin skill nueva.**
Decisión D10 de `plans/PLAN-007.md`. La bitácora es, para el enrutador, otra forma de «apuntar y
recuperar algo personal», que es exactamente el dominio de `recuerdos` —la misma skill que ya cubre
`notes` y `unified_search`—. Estrenar una skill obligaría a repartir el habla de «diario» entre dos
candidatas que se solapan (y `timeline` ya reivindica el «diario de actividad»), con el riesgo de que
ninguna superase su umbral. Sus `criteria` se amplían para incluir el habla propia de la bitácora
(«bitácora», «diario», «cómo fue mi día»). El efecto en el enrutado se **mide** con el arnés del
repositorio (`src/bin/route-eval.rs`) antes y después de tocar los criterios, y queda como tarea de
verificación: no se cambia la granularidad de las skills ni los umbrales.

**D6 — Sin resumen semanal ni clima, y sin conectar con el timeline.**
El resumen semanal (`JournalDigest`), la herramienta de «insights» y el `weather_context` se
descartan del diseño original (ver `plans/PLAN-007.md` — «Qué se descarta» y Non-goals del
proposal). Si el tiempo del día importa, saldrá en el relato. La bitácora tampoco lee ni escribe el
timeline: el riesgo de redundancia (una bitácora que solo cuenta hechos) se vigila con el prompt del
change siguiente, no aquí.

**D7 — La API distingue «mal formado» (`400`) de «no existe» (`404`).**
Una fecha que no encaja en `YYYY-MM-DD` o un ánimo fuera del conjunto son `400` con el motivo: son
peticiones mal formadas, no ausencias. Un día sin entrada es `404`, tanto en `GET /api/journal/{date}`
(no hay nada que devolver) como en `DELETE /api/journal/{date}`: el diario del usuario no debe
responder «borrado» a algo que nunca estuvo. Se descarta el `204` idempotente de otras rutas porque
aquí la ausencia es información útil para el llamante. `PUT` crea o reemplaza y devuelve la entrada
resultante (upsert), de modo que la app y el agente usan el mismo camino.

**D8 — La vista de Bitácora es una sección propia, no un tab de ajustes.**
Es una vista operativa (leer, editar, borrar), como `TaskView`, `CalendarView` y `NotificationsView`,
no una preferencia. Se abre desde una entrada en la navegación del header de `AppLayout`, se carga
de forma diferida (`React.lazy` + `Suspense`) y muestra los estados de carga, vacío y error como las
demás. Los textos van en español y los tipos en `frontend/src/types/`.

## Risks / Trade-offs

- **Redundancia con el timeline** → es el riesgo que puede hundir la feature (D6). Aquí no se
  resuelve: se acota dejando fuera cualquier síntesis y se vigila con el prompt y con entradas
  reales al cerrar `journal-worker`.
- **La fecha en la zona del usuario** (cambios de hora, zonas raras) → el repositorio guarda la
  fecha tal cual y no la recalcula; el cálculo del día local es del worker (change siguiente), que
  lo probará a conciencia y caerá a UTC sin ajuste válido.
- **`updated_at` con granularidad de segundo** → dos escrituras en el mismo segundo no se
  distinguirían por fecha; no afecta al contrato (la unicidad la da la fecha, no `updated_at`), pero
  se anota para no apoyar ninguna lógica de orden en `updated_at`.
- **Sin `CHECK` de ánimo en la base** (D1) → una escritura SQL directa podría meter un valor
  inválido; se acepta porque el único camino de escritura es el repositorio y la migración se
  mantiene simple y aditiva.

## Migration Plan

Migración aditiva `migrations/20261018000001_journal_entries.sql` con `CREATE TABLE IF NOT EXISTS`
y su índice `IF NOT EXISTS`: reejecutable y sin datos que migrar (la tabla nace vacía). Despliegue
habitual (`just dev` para el contenedor de trabajo). Rollback: revertir el commit; al no haber datos
de producción, no hay estado que deshacer.
