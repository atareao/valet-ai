# Tasks

## 1. RED: fijar el diseño medido como contrato

- [x] 1.1 Tests del catálogo nuevo que **fallan** con el actual: seis skills (`agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets`), `pendientes` cubre `tasks` y `reminders`, `recuerdos` cubre `notes` y `unified_search`, `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`; el core es `get_current_time` y `get_current_location`; `render_widget` **no** está en el core; cobertura 13/13.
- [x] 1.2 Test que falla: cada skill declara su umbral por defecto (dominios 0,10; `widgets` 0,20) y `read_router_config` aplica el override `ROUTER_THRESHOLD_<ID>` cuando existe.
- [x] 1.3 Tests que fallan sobre los criterios efectivos: fila ausente → valor compilado; override presente → se usa; fila vacía en blanco → valor compilado (**nunca** un criterio vacío).

## 2. GREEN: catálogo de seis dominios

- [x] 2.1 `src/orchestrator/skills.rs`: enum y catálogo con los seis dominios, sus herramientas, el core reducido, y por skill el id, las `criteria` compactas por defecto, el umbral por defecto, y la clave/encabezado del fragmento.
- [x] 2.2 Criterios **compactos** (los medidos): condiciones observables y negativo que nombra lo que el mensaje es en su lugar.
- [x] 2.3 Verificar que el test de integridad sigue exigiendo 13/13 y que ningún nombre antiguo (`tareas`, `recordatorios`, `notas`, `clima`, `lugares`, `busqueda_web`, `memoria`) queda en el código.

## 3. Criterios efectivos en settings

- [x] 3.1 `read_skill_criteria(pool, skills)` en `skill_router.rs`: lee `SKILL_<ID>_QUESTION`, `_CRITERIA_TRUE` y `_CRITERIA_FALSE` (una sola lectura de la tabla), aplica el override y cae al valor compilado si falta o está en blanco.
- [x] 3.2 `select` usa los criterios efectivos en la construcción de cada pregunta; un criterio efectivo vacío es imposible por construcción.
- [x] 3.3 Tests: override aplicado en el mismo turno, ausencia → compilado, blanco → compilado, error de BD → compilados + warning.

## 4. Configuración: umbral por skill y turnos de historial

- [x] 4.1 Migración idempotente que ajusta `ROUTER_THRESHOLD` a `0.10`, añade `ROUTER_THRESHOLD_WIDGETS` a `0.20` y sube `ROUTER_HISTORY_TURNS` a `6`, **sin sobrescribir** un valor existente del usuario.
- [x] 4.2 `read_router_config` lee el override por skill (`ROUTER_THRESHOLD_<ID>`) y lo aplica por encima del umbral global; el global sigue siendo el default de las skills sin override.
- [x] 4.3 Tests de migración: claves y valores tras migrar; re-ejecución que no pisa un valor editado; override ilegible → umbral por defecto de la skill + warning.

## 5. La guía de widgets se muda a su fragmento

- [x] 5.1 Migración que retira de `settings.system_prompt` el bloque de widgets **solo si está verbatim** (identificado por su encabezado) y lo deposita en `SKILL_WIDGETS_PROMPT`; idempotente y sin tocar el resto del texto del usuario.
- [x] 5.2 Si el bloque no está verbatim (editado o ausente): no se retira y el fragmento se siembra igualmente; la regla anti-duplicado del ensamblador evita la duplicación.
- [x] 5.3 Tests de migración para los tres casos: verbatim (se retira y se deposita), editado (no se retira, no se duplica), ausente (idempotente).

## 6. API del catálogo con valores efectivos

- [x] 6.1 `/api/skills` devuelve por skill: id, herramientas, `question`, `criteria_true`, `criteria_false`, umbral **efectivo** y qué campos están **sobrescritos**.
- [x] 6.2 Tests: valores por defecto cuando no hay overrides; valores sobrescritos y marca cuando los hay.

## 7. Arnés: configuración efectiva y re-medición

- [x] 7.1 El informe publica las skills, los umbrales efectivos y qué criterios están sobrescritos, para que cada ejecución sea atribuible.
- [x] 7.2 Tests de la parte pura (`effective_config`).
- [x] 7.3 **Re-medición real** sobre los 66 turnos con el diseño nuevo y registro de la cifra en el PR.
  - Medido: **cobertura 92,4-95,5 % en cinco ejecuciones** (varianza ±2-3 pp), latencia p50 313-375 ms / p95 ~801 ms, coste ~$0,00003 por turno, 2,27 skills activadas por turno. Los fallos restantes son **continuaciones, correcciones y saludos** (clase inalcanzable para cualquier clasificador con esta métrica).
  - **PARCIAL**: el arnés instrumenta cobertura, latencia, coste y activaciones; **no** instrumenta herramientas por turno ni bytes ahorrados (el 28,1 % del diseño procede del análisis con la misma configuración). Queda pendiente instrumentarlo.
  - **El criterio de aceptación de ≥97 % NO se cumple**: se fijó como dintel puntual sobre una medida con ruido de ±2-3 pp. El enrutador se entrega **apagado**.

## 8. UI

- [x] 8.1 «Herramientas»: el control muestra el umbral **global** y el **efectivo por skill**, con el override editable para la skill que lo tenga.
- [x] 8.2 «Prompts» → «Skills»: una tarjeta por skill **listada desde el catálogo** (no por patrón de clave) con la pregunta, los dos criterios y el fragmento; marca y acción de «restaurar por defecto» en los campos sobrescritos.
- [x] 8.3 Aviso no bloqueante si un campo queda vacío al guardar (se usará el valor por defecto).
- [x] 8.4 Tests de componentes para 8.1-8.3, incluida la lista desde el catálogo y el «restaurar».

## 9. Cierre

- [x] 9.1 `just check-all` en verde (946 tests en todos los targets, 0 fallos; 316 de frontend).
- [x] 9.2 Reviews con `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
  - **`rust-reviewer`**: 1 MAYOR real (la guía de widgets podía viajar dos veces si el usuario había editado el bloque: el marcador anti-duplicado miraba un encabezado que el fragmento no llevaba) + 4 menores (marca `overridden` del umbral, trim consistente, validación de `--threshold` en el arnés) — todo con test. Verificaciones pendientes completadas, incluida la **discrepancia de contexto entre el arnés y producción**.
  - **`react-reviewer`**: sin bugs críticos; 2 mayores aplicados (colisión de nombres accesibles entre las seis skills y doble `GET /api/skills` por apertura, ahora con un único dueño del catálogo) + menores (tipo unión de `overridden`, tests directos de `skillEffectiveValues`/`skillFields`, test del fallo de «Restaurar»). Deuda documentada en código: restaurar un fragmento (hoy inalcanzable) y un override de umbral igual al global (invisible, no limpiable).
- [ ] 9.3 Documentar en el PR la medición antes/después y la decisión razonada sobre encender (el enrutador se entrega **apagado**).
- [ ] 9.4 PR a `development` y `openspec archive skill-router-tuning`.
