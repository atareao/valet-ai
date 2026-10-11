# Tasks

## 0. Preparación y aprobación

- [ ] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [x] 1.1 Tests que fallan (migración): tras la migración nueva, `timeline_events` tiene la columna `location` (admite `NULL`) y el índice único sobre `(lower(trim(fact)), substr(timestamp,1,10))`; el mismo hecho el mismo día lo rechaza la base de datos; el mismo hecho en días distintos coexiste; `'  corrí 9,34 km  '` choca con `'Corrí 9,34 km'`.
- [x] 1.2 Tests que fallan (vaciado): con filas previas la migración deja `timeline_events` **sin ninguna fila** y `messages` intacta; con duplicados previos termina sin error y el índice único queda creable.
- [x] 1.3 Tests que fallan (prompts): la migración reemplaza `timeline_prompt` y `SKILL_TIMELINE_PROMPT` cuando su valor es el sembrado, y **no** los toca cuando el valor es otro; el prompt sembrado nombra las exclusiones (órdenes al asistente, estados transitorios, opiniones, preguntas, planes futuros), dice que las fases de un mismo acontecimiento son un solo hecho, pide `timestamp` y `location`, y ordena no repetir los hechos ya listados ni con las mismas palabras ni con otras.
- [x] 1.4 Tests que fallan (parser): `timestamp` y `location` se leen del JSON con tolerancia (ausentes o mal tipados → `None`) sin romper el resto del hecho.
- [x] 1.5 Tests que fallan (fuente): un hecho cuyo `source` señala una línea de rol `assistant`, `tool` o `system` se descarta; uno que señala una línea `user` se persiste; el `source_message_id` es siempre el `id` de esa línea.
- [x] 1.6 Tests que fallan (fecha): un `timestamp` válido se respeta; uno ausente, ilegible, sin desfase horario, en el futuro más allá de la tolerancia o anterior a la ventana admitida cae al `created_at` del mensaje citado.
- [x] 1.7 Tests que fallan (lugar y sustancia): un `location` vacío o en blanco queda a `NULL`; uno de más de 120 caracteres se descarta; un `fact` de más de 280 caracteres se descarta por no ser atómico; el resto de hechos del lote se persiste igual.
- [x] 1.8 Tests que fallan (dedup entre pasadas): un hecho que ya existe en la tabla para ese día no se inserta otra vez y la pasada no falla; `TimelineRepo::insert_many` devuelve cuántas filas entraron de verdad.
- [x] 1.9 Tests que fallan (bloque del prompt): el bloque lleva el número, el `created_at` y el **rol** de cada mensaje, el instante de referencia del lote y la lista de hechos ya registrados de los últimos días.
- [x] 1.10 Tests que fallan (tools): `timeline_get_events` devuelve `location`; `timeline_add_event` acepta `location`, la valida igual que el extractor y responde que el hecho ya existía cuando la inserción se ignora.
- [x] 1.11 Tests que fallan (integridad): la suite completa sigue en verde y los recuentos de skills y herramientas no cambian (10 y 28).

## 2. GREEN

- [x] 2.1 Migración nueva `migrations/20261011000001_timeline_fact_integrity.sql`: `DELETE FROM timeline_events`, `ALTER TABLE … ADD COLUMN location TEXT`, `CREATE UNIQUE INDEX` sobre `(lower(trim(fact)), substr(timestamp,1,10))` y la actualización guardada de `timeline_prompt` y `SKILL_TIMELINE_PROMPT`.
- [x] 2.2 `src/models/timeline.rs`: `location: Option<String>` en `TimelineEvent` y en `NewTimelineEvent`.
- [x] 2.3 `src/db/repos/timeline.rs`: `location` en el `SELECT` del listado, `INSERT OR IGNORE` en `insert_many` devolviendo `rows_affected`, y el listado acotado que alimenta la lista de hechos conocidos.
- [x] 2.4 `src/workers/episodic_memory.rs`: `RawTimelineEvent` con `timestamp` y `location`; `format_timeline_block` con el rol, el instante de referencia y los hechos conocidos; `validate_timeline_events` con el descarte por rol, la validación de fecha, los topes de longitud y el dedup; y el respaldo mínimo del prompt alineado con el contrato nuevo.
- [x] 2.5 `src/tools/timeline.rs`: `location` en el esquema y en la descripción de `timeline_add_event`, `location` en lo que devuelve `timeline_get_events`, y el mensaje de «ya existe» cuando la inserción se ignora.

## 3. REFACTOR / limpieza

- [x] 3.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` limpios; sin código muerto ni variantes sin usar.
- [x] 3.2 Comprobar que ningún test afirma ya el comportamiento viejo (fecha tomada siempre del mensaje, `source` de cualquier rol) y que las specs no se contradicen entre sí.

## 4. VERIFICACIÓN

- [ ] 4.1 Anotar el recuento de tests **antes y después** y dejar `cargo test --no-fail-fast` en verde (0 fallos). Antes: 1208 passed / 0 failed / 1 ignored.
- [ ] 4.2 `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios.
- [ ] 4.3 PR a `development` con merge commit, revisado por `rust-reviewer`.
- [ ] 4.4 En producción, reproducir **el caso que lo rompió**: pedir ver el timeline y comprobar que la tabla no crece, que el hecho nuevo lleva fecha y lugar cuando el mensaje los da, que una tirada contada en dos pasos deja **un solo** hecho y que ningún hecho cita un mensaje del asistente.

## Notas

- **Se vacía la tabla, no se repara.** El timeline es dato derivado: lo escribió el extractor y el material crudo sigue en `messages`. Vaciarlo evita el único SQL delicado del change (un borrado determinista de duplicados que conservase la fila más antigua) y deja el índice único creable sin condiciones.
- **La ventana de fechas admitida y los topes de longitud son constantes con nombre** en `episodic_memory.rs`, no números sueltos: 7 días hacia atrás, 5 minutos de tolerancia hacia el futuro, 120 caracteres de lugar y 280 de hecho.
- **El grano del hecho es best-effort.** Que `Salí a correr 10 km` y `Terminé el entrenamiento` sean un solo hecho lo decide el modelo leyendo la lista de hechos ya registrados; ningún índice lo puede resolver. El prompt lo dice y pone el ejemplo, y eso es todo lo que se puede garantizar.
- **El índice único normaliza con `lower()` y `trim()`**, que en SQLite solo afectan al ASCII: es consistente porque el mismo valor se normaliza igual al insertar y al comparar. Las tildes no se normalizan, así que `Cené` y `Cene` son hechos distintos; se acepta.
- **La clave del índice es el día, no el instante**: el mismo hecho en días distintos es un hecho distinto (cenar mochis el domingo y el lunes son dos cenas). El mismo hecho el mismo día es siempre un duplicado.
- **`INSERT OR IGNORE` es deliberado**: la violación del índice único es el resultado esperado de la deduplicación, no un error que deba abortar la pasada. `insert_many` devuelve cuántas filas entraron para que `timeline_add_event` pueda decírselo al modelo.
- **Los hechos conocidos se leen de la tabla**, no del lote: es lo que permite cazar un casi-duplicado que llega en una pasada distinta de la del hecho original.
- **La sustancia de un hecho no es comprobable en código.** El prompt sembrado es el contrato y tiene sus propios tests (contiene las reglas); más allá de eso, el único filtro mecánico es el tope de 280 caracteres.

### GREEN — lo que de verdad pasó (desviaciones)

- **1.1 / 1.2 / 1.3 — un test preexistente chocaba con el índice único.** `test_timeline_events_category_check_roundtrip` (que el RED no tocó) insertaba 7 hechos con `fact = 'hecho'` y el mismo día, uno por categoría, para comprobar el `CHECK`; con el índice `(lower(trim(fact)), substr(timestamp,1,10))` en pie, del 2º insert en adelante falla (reproducido con `sqlite3`). Se le dio a cada fila un `fact` distinto (`hecho {i}`) — **ninguna aserción cambió** — con aprobación explícita del usuario. Es el único test preexistente afectado (revisados todos los `INSERT INTO timeline_events` de `tests/` y `src/`).
- **1.3 / 2.1 / 2.4 — el prompt «exacto» no contenía un literal que el RED exige.** El texto entregado decía `NO extraigas órdenes ni peticiones dirigidas al asistente`, que no contiene el literal `órdenes dirigidas al asistente` de `NEW_PROMPT_RULES`; como las reglas prohíben tocar los tests, se reescribió la línea a `NO extraigas órdenes dirigidas al asistente ni peticiones dirigidas a él (…)` (mismo significado; mantiene `un solo hecho`, `estados transitorios`, `ISO 8601`, `location` y `hechos ya registrados`, y no reintroduce `NO devuelvas ninguna fecha`). Es la única desviación del texto del prompt.
- **2.4 — `DEFAULT_TIMELINE_PROMPT_FALLBACK`.** Reescrito como *raw string* coherente con el contrato nuevo; ningún test fija su contenido (solo se compara consigo mismo), así que no está constreñido por el RED.
- **Verificación.** `cargo test --no-fail-fast` → **1254 passed / 0 failed / 1 ignored** (base 1208 + 46 del RED). `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios (exit 0).

### REFACTOR — hallazgos del review y cómo se cerraron

Dos hallazgos importantes del `rust-reviewer` (cero bloqueantes). Ambos cerrados sin tocar la migración ni los textos de los prompts, sin debilitar ninguna aserción existente.

- **H1 — `INSERT OR IGNORE` enmascaraba violaciones que no eran duplicados.** Con `INSERT OR IGNORE` una violación del `CHECK` de categoría se saltaba en silencio (solo la del índice único debía ignorarse), y `timeline_add_event` la reportaba como «ya estaba registrado». Se cambió a `INSERT … ON CONFLICT(lower(trim(fact)), substr(timestamp, 1, 10)) DO NOTHING`: el conflicto queda acotado a la unicidad, el duplicado sigue siendo un no-op (semántica de lote y `rows_affected` intactos) y una violación real (CHECK/FK) pasa a ser un error ruidoso. La consecuencia asumida —una violación de CHECK/FK aborta la transacción de la pasada— queda documentada en el doc de `insert_many`: es aceptable porque la Capa D es *best-effort*, la validación previa hace esas violaciones inalcanzables y la pasada se reintenta.
- **H2 — faltaba el test del escenario S3 («Sin hechos no hay filas nuevas»).** Solo estaba cubierto el parser (`test_parse_timeline_events_empty_array`); los dos tests de best-effort existentes usaban `""` y contenido sin JSON, o sea la rama de error. Se añadió `test_evaluate_timeline_empty_events_still_persists`, end-to-end sobre la pasada: con el extractor devolviendo `{"events": []}` no se inserta ninguna fila, el lote queda con `is_indexed = 1`, la ficha y el estado se escriben y la estadística de la llamada queda con `kind = 'timeline'` y `status = 'success'` (y no `'error'`).
- **Menor cerrado — S17 (el prompt siembra el ejemplo).** `assert_new_prompt_contract` solo comprobaba `un solo hecho`; se añadieron las subcadenas del ejemplo (`Salir a correr` y `terminar el entrenamiento`) a `NEW_PROMPT_RULES`. El helper solo se usa para el prompt del **extractor**; el de la skill se afirma aparte y no lleva el ejemplo, así que no se contamina.
- **Verificación.** `cargo test --no-fail-fast` → **1255 passed / 0 failed / 1 ignored** (1254 + el test nuevo de S3). `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios (exit 0).
