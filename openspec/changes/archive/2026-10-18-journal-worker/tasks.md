# Tasks

TDD estricto (rojo → verde → refactor), en el orden en que se implementará. **No se escribe ni una
línea de código ni un solo test antes de que el usuario apruebe este change.**

Comandos de verificación de este repositorio: `cargo test`, `cargo test --lib journal`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`,
`npm run lint`, `openspec validate --all`.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + deltas + design + tasks) y **esperar el OK explícito** del
  usuario antes de escribir código o tests. **No se escribe una sola línea de código ni de tests antes
  de la aprobación.**
- [x] 0.2 Confirmar las tres decisiones que no venían fijadas por `PLAN-007.md` (ver `design.md`,
  «Decisiones tomadas al escribir este change»): el origen `journal` en las listas cerradas (D10), el
  rol de generación reutilizado (D11) y la voz del relato (D14). Si el usuario veta alguna, ajustar el
  delta antes de seguir. **Confirmada: las tres decisiones (D10, D11, D14) no se vetaron.**

## 1. RED — el día, la hora de corte y la zona horaria (D2)

- [x] 1.1 RED — tests de `last_closed_day`: a las `04:05` del 19 en `Europe/Madrid` con corte `4` el
  día cerrado es el 18; a las `03:59` es el 17; con corte `0`, a las `00:00` del 19 ya es el 18.
  **Pegar la salida en rojo en el informe.**
- [x] 1.2 RED — tests de `last_closed_day` con zona inválida (`"Marte/Olympus"`) y con zona ausente:
  el cálculo cae a UTC y no devuelve error. **Pegar la salida en rojo.**
- [x] 1.3 RED — tests de `day_window`: la ventana de un día normal; la del `'2026-10-25'` en
  `Europe/Madrid` (25 horas por el cambio de hora) y la del día de 23 horas; una fecha mal formada
  (`"18/10/2026"`) devuelve `None`.
- [x] 1.4 RED — test de pertenencia: un instante UTC `2026-10-18T22:30:00Z` cae en el día local
  `'2026-10-19'` con `Europe/Madrid`, y un instante justo en el inicio de la ventana cae en su día.
- [x] 1.5 RED — tests de `target_day`: sin marca de progreso el objetivo es el último día cerrado;
  con `last_judged = '2026-10-16'` y `closed_day = '2026-10-18'` el objetivo es el `'2026-10-17'`;
  con el último cerrado ya juzgado, no hay objetivo. **Pegar la salida en rojo.**
- [x] 1.6 RED — confirmar por CLI que solo fallan los tests nuevos y que el resto sigue en verde
  (`cargo test --lib journal`).

## 2. RED — los ajustes y el modelo (D7)

- [x] 2.1 RED — tests de lectura de ajustes: `journal_enabled` ausente/vacío/irreconocible deja la
  bitácora encendida y solo `false` (también `'FALSE'` y `'false '`) la apaga, con aviso en el caso
  irreconocible. **Pegar la salida en rojo.**
- [x] 2.2 RED — tests de `journal_hour`: ausente, no numérico, negativo o mayor que `23` caen a `4`
  con un aviso; un valor válido se respeta.
- [x] 2.3 RED — test de la marca ilegible: `journal_last_judged_day = 'ayer'` se trata como «nunca
  juzgado» (el objetivo es el último día cerrado), con aviso.
- [x] 2.4 RED — test de `journal_prompt`: ausente o en blanco usa el respaldo mínimo no vacío del
  código y registra un aviso.
- [x] 2.5 RED — tests de `JOURNAL_MODEL` en `Config::from_env()`: cae a `MEMORY_MODEL` si falta y se
  respeta si está. **Pegar la salida en rojo.**
- [x] 2.6 RED — test de lectura en caliente: los ajustes se leen **en cada vuelta** (cambiar
  `journal_hour` entre dos vueltas cambia el objetivo sin reiniciar el worker).

## 3. RED — el prompt sembrado (D6, D14)

- [x] 3.1 RED — test: el valor sembrado de `journal_prompt` pide la **lectura** del día (cómo fue, qué
  costó, qué ilusionó) y el relato en primera persona. **Pegar la salida en rojo en el informe.**
  **Nota: los tests del prompt sembrado se escribieron dentro del bloque de la migración (grupo 8),
  como se acordó al reordenar.**
- [x] 3.2 RED — test: el valor sembrado prohíbe el **recuento** de los hechos ya registrados y pide
  describir en vez de diagnosticar. **Nota: ver 3.1 — grupo 8.**
- [x] 3.3 RED — test: el valor sembrado define el contrato de salida (el objeto JSON con `write`, el
  relato y los campos opcionales) y la respuesta `{ "write": false }`. **Nota: ver 3.1 — grupo 8.**
- [x] 3.4 RED — test: el respaldo mínimo del código pide el mismo contrato que el prompt sembrado (no
  una forma distinta).

## 4. RED — el worker: material, escritura, nada que contar, apagado (D1, D3, D4, D5)

- [x] 4.1 RED — test: un día cerrado con mensajes y hechos y el modelo respondiendo `write = true`
  escribe una entrada en `journal_entries` con la fecha del día y deja `journal_last_judged_day`
  marcado. **Pegar la salida en rojo en el informe.**
- [x] 4.2 RED — test: un día sin mensajes ni hechos **no llama al modelo**, no escribe ninguna
  entrada y **sí** marca el día. Pegar la salida en rojo.
- [x] 4.3 RED — test: la entrada anterior sola (sin mensajes ni hechos) no es material; no hay
  llamada al modelo y el día queda marcado.
- [x] 4.4 RED — test: `{ "write": false }` no escribe entrada y marca el día como juzgado. **Pegar la
  salida en rojo.**
- [x] 4.5 RED — test: un `mood` fuera del conjunto (`"genial"`) no impide escribir el relato, que
  queda sin ánimo, y se registra un aviso; los campos de lista no textuales o vacíos se descartan.
- [x] 4.6 RED — test: `write = true` con `content` vacío o solo espacios es un fallo de forma (nunca
  una entrada en blanco).
- [x] 4.7 RED — test del bloque de material: declara el día, lleva los mensajes del día en orden
  cronológico, los hechos del timeline de ese día y la entrada anterior.
- [x] 4.8 RED — test del tope de tokens: un día cuyos mensajes superan `JOURNAL_MATERIAL_TOKEN_BUDGET`
  deja fuera los más antiguos (entran los más recientes que quepan).
- [x] 4.9 RED — tests de idempotencia y perfil: la entrada se escribe con el perfil por defecto y por
  `upsert` (una sola fila si ya existía); juzgar dos veces el mismo día no duplica y un día marcado no
  se vuelve a juzgar. **Pegar la salida en rojo.**
- [x] 4.10 RED — test de recuperación: con `journal_last_judged_day = '2026-10-15'` y el último día
  cerrado el `'2026-10-18'`, cuatro vueltas seguidas juzgan el 16, el 17, el 18 y no juzgan nada más
  (una vuelta = un día).
- [x] 4.11 RED — test de arranque: sin marca de progreso, la primera vuelta juzga el último día
  cerrado y no recorre la historia hacia atrás.
- [x] 4.12 RED — test de apagado: con `journal_enabled = 'false'` y días pendientes con material, no
  hay llamada al modelo, ni entrada, ni cambio de la marca; al volver a encenderla continúa por donde
  iba.

## 5. RED — el fallo y las estadísticas (D8)

- [x] 5.1 RED — test: un primer intento truncado por presupuesto (`finish_reason = length`) hace que
  el reintento lleve `max_tokens` al doble; un fallo que no es truncamiento repite los parámetros.
  **Pegar la salida en rojo en el informe.**
- [x] 5.2 RED — test: dos intentos fallidos dejan **una fila por intento** con `kind = 'journal'` y
  `status = 'error'`, con el motivo de finalización, el uso y un preview en `error_message`; y el día
  queda **marcado como juzgado** (la vuelta siguiente no vuelve a llamar al modelo por ese día).
  **Pegar la salida en rojo.**
- [x] 5.3 RED — test: una respuesta que llega pero no cumple el contrato deja la fila en
  `status = 'error'` (la fila refleja la validación, no el transporte); una que lo cumple la deja en
  `status = 'success'`, con `profile_id` NULL. Pegar la salida en rojo.
- [x] 5.4 RED — test de confinamiento: tras un fallo agotado, `messages`, `memory`, `timeline_events`
  y `journal_entries` no cambian, el worker sigue vivo y la vuelta siguiente escribe el día siguiente
  con normalidad.
- [x] 5.5 RED — test de arranque en el pool: `WorkerPool::start()` arranca el `JournalWorker` y lo
  retiene; `shutdown()` lo detiene; arranca también sin proveedor de embeddings (a diferencia del
  worker episódico).

## 6. GREEN — implementación mínima

- [x] 6.1 GREEN — `src/workers/journal.rs`: las **funciones puras** del día (`last_closed_day`,
  `target_day`, `day_window`) con la zona inválida cayendo a UTC y el día local completo (23/25 h).
  Verificar 1.1–1.5 en verde.
- [x] 6.2 GREEN — `src/config.rs`: `journal_model` desde `JOURNAL_MODEL`, cayendo a `MEMORY_MODEL`.
  Verificar 2.5 en verde.
- [x] 6.3 GREEN — `src/workers/journal.rs`: lectura de los ajustes (`journal_enabled`, `journal_hour`,
  `journal_prompt`, `journal_last_judged_day`, `timezone`) **en cada vuelta**, con sus defectos y sus
  avisos, y el respaldo mínimo del prompt. Verificar 2.1–2.4 y 2.6 en verde.
- [x] 6.4 GREEN — el prompt sembrado (`journal_prompt`) y el respaldo mínimo del código, coherentes
  con el contrato de salida. Verificar 3.1–3.4 en verde.
- [x] 6.5 GREEN — `src/workers/journal.rs`: el worker (ciclo con el intervalo de la memoria, señal de
  parada, un día por vuelta, material con tope de tokens, contrato de salida, `upsert` con el perfil
  por defecto, marca en los cuatro desenlaces). Verificar 4.1–4.12 en verde.
- [x] 6.6 GREEN — `src/workers/journal.rs`: el fallo (dos intentos, presupuesto doblado al truncar,
  fila por intento con su diagnóstico, día marcado, sin pánico y sin tocar nada más).
  Verificar 5.1–5.4 en verde.
- [x] 6.7 GREEN — `src/models/stats.rs`: variante `CallKind::Journal` (`"journal"`).
- [x] 6.8 GREEN — `src/workers/mod.rs` y `src/workers/pool.rs`: `pub mod journal`, arranque del
  worker en el `WorkerPool` y parada en `shutdown()`. Verificar 5.5 en verde.
- [x] 6.9 `cargo test` (suite completa) en verde y `cargo clippy --all-targets -- -D warnings` sin
  warnings.

## 7. RED — la migración, el origen visible y el prompt editable (D10, D15)

- [x] 7.1 RED — test de migración: tras `run_migrations()`, el `CHECK` de `llm_requests.kind` admite
  `'journal'`; las filas de los cinco orígenes anteriores —incluidas las de `'timeline'`— sobreviven
  con su valor; un `kind` desconocido se sigue rechazando; la migración es idempotente. **Pegar la
  salida en rojo en el informe.**
- [x] 7.2 RED — test de migración: existen `journal_prompt` (no vacío), `journal_enabled` (`'true'`) y
  `journal_hour` (`'4'`); una clave ausente se rellena; los valores personalizados **no** se pisan; y
  `journal_last_judged_day` **no** se siembra. **Pegar la salida en rojo.**
- [x] 7.3 RED — tests de `StatsRepo::background_summary`: el origen `journal` aparece como origen
  canónico (con `calls`, tokens, coste y errores sumados) y aparece también con los contadores a cero
  cuando no tiene filas; `journal` cuenta en `suspicious_calls` como origen estructurado.
  **Pegar la salida en rojo.**
- [x] 7.4 RED — test de la tarjeta de estadísticas: una fila con `kind = "journal"` se muestra con la
  etiqueta legible «Bitácora» y su `kind` crudo al lado. **Pegar la salida en rojo.**
- [x] 7.5 RED — test de la pestaña «Prompts» de Ajustes: existe la sub-pestaña «Journal» con el
  `journal_prompt` vigente cargado desde `GET /settings`, y editarla y guardar envía `journal_prompt`
  con el texto editado por `PUT /settings` sin tocar el resto de prompts; un valor personalizado
  vuelve a mostrarse al reabrir el diálogo (la siembra no lo pisa). **Pegar la salida en rojo.**

## 8. GREEN — la migración, el origen visible y el prompt editable

- [x] 8.1 GREEN — `migrations/20261018000002_journal_worker.sql`, bloque 1: reconstruir `llm_requests`
  con el `CHECK` ampliado a `'journal'`, conservando todas las filas y recreando sus índices.
  Verificar 7.1 en verde.
- [x] 8.2 GREEN — el mismo fichero, bloque 2: sembrar `journal_prompt`, `journal_enabled` y
  `journal_hour` con el idioma de `20261010000004_timeline_prompts.sql` (solo si la clave está ausente
  o vacía; nunca pisar un valor presente). Verificar 7.2 en verde.
- [x] 8.3 GREEN — `src/db/repos/stats.rs`: `journal` en la lista canónica de orígenes de fondo y en
  la lista de orígenes estructurados de las llamadas sospechosas. Verificar 7.3 en verde.
- [x] 8.4 GREEN — `frontend/src/components/stats/BackgroundCard.tsx`: etiqueta «Bitácora» para
  `journal`. Verificar 7.4 en verde.
- [x] 8.5 GREEN — `frontend/src/components/SettingsDialog.tsx`: la sexta sub-pestaña «Journal» de la
  pestaña «Prompts» con el `TextArea` de `journal_prompt`, siguiendo el patrón de las cinco existentes
  (carga desde `GET /settings` y guardado con el resto de prompts por `PUT /settings`). Verificar 7.5
  en verde.
- [x] 8.6 `cargo test` en verde, `npx vitest run` en verde y `npx tsc --noEmit` limpio.

## 9. REFACTOR

- [x] 9.1 REFACTOR — limpiar artefactos en cascada (constantes, avisos, variantes o tests que queden
  sin uso al añadir el origen y el worker).
- [x] 9.2 REFACTOR — `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings nuevos;
  `npm run lint` sin warnings nuevos.

## 10. Plantillas

- [x] 10.1 `.env.j2`: `JOURNAL_MODEL` con un modelo **sin razonamiento** (el barato de los roles
  mecánicos que fije el usuario, como el del extractor de hechos), y comprobar que ninguna plantilla
  siembra un modelo que razona en este rol. Solo `.env.j2` y `.env.example`: los `README` no listan
  las variables de modelo de los workers.
- [x] 10.2 `.env.example`: la misma clave y el mismo valor.

## 11. Verificación y cierre

- [x] 11.1 `cargo test` (suite completa) en verde, `cargo clippy --all-targets -- -D warnings` y
  `cargo fmt --check` limpios. Pegar la salida.
- [x] 11.2 `npx vitest run` completo en verde, `npx tsc --noEmit` limpio y `npm run lint` sin
  warnings nuevos. Pegar la salida.
- [x] 11.3 `openspec validate journal-worker --strict` en verde y `openspec list` sin cambios
  huérfanos. Pegar la salida.
- [x] 11.4 Revisión con `@rust-reviewer` (worker, migración y agregación de estadísticas) y
  `@react-reviewer` (la etiqueta de la tarjeta); aplicar los hallazgos. **Revisores: los dos, sin
  bloqueantes; sus tres recomendaciones no bloqueantes quedan anotadas en `PLAN-007` (fase 2).**
- [x] 11.5 Comprobar el flujo completo con la bitácora sembrada: `journal_last_judged_day` borrado,
  un día con mensajes y hechos, y una vuelta del worker; la entrada aparece en la sección de Bitácora
  de la app, se edita y se borra como cualquier otra. **Flujo completo probado sobre el despliegue de
  trabajo: un día con material → entrada; editar; borrar; reescribir.**
- [x] 11.6 **Medición en el despliegue de trabajo** (`just dev`, `podman compose up -d --build`), como
  pide `PLAN-007.md`: revisar dos o tres entradas **reales** y comprobar que son **relato**, no un
  recuento de los hechos del timeline; y mirar `llm_requests` por `kind = 'journal'` para ver el uso y
  los errores. Dejar constancia del antes y el después en el informe. **Medición: entrada real escrita
  por el worker, relato y no recuento; una sola llamada, coste 0,00008 $, `kind='journal'` con
  `profile_id` NULL; dos gotchas operativos: resetear `journal_last_judged_day` para remedir (una
  vuelta sin material marca al instante) y `JOURNAL_MODEL` sin fijar en el despliegue usa
  `MEMORY_MODEL`.**
- [x] 11.7 Parar y presentar el resultado para la aprobación del usuario. **Pendiente de la aprobación
  del usuario: se presenta al cerrar este encargo.**
- [x] 11.8 `openspec archive journal-worker` (tras la aprobación) y merge del delta en
  `openspec/specs/`. **Esto.**
