# Tasks

## 1. Baseline y caracterización (RED)

- [x] 1.1 Test que serializa las definiciones del registry de producción y registra su tamaño en tokens con `token_estimate`: es la referencia del «antes» (13 esquemas, ~3,5-4 k tokens).
- [x] 1.2 Test de caracterización del comportamiento de hoy: la petición al LLM ofrece las 13 herramientas habilitadas. Debe quedar **en verde** antes de tocar nada.

## 2. Catálogo de skills (dominio)

- [x] 2.1 Nuevo `src/orchestrator/skills.rs`: `enum Skill` cerrado y `SkillSpec` (id estable, instrucciones para Jev, `criteria` de true y false, conjunto de tools, clave y encabezado del fragmento), con `catalog()` para las ocho skills.
- [x] 2.2 `CORE_TOOLS` = `render_widget`, `get_current_time`.
- [x] 2.3 `tools_for(&[Skill]) -> Vec<String>`: unión sin duplicados y en orden determinista.
- [x] 2.4 `skill_of_tool(&str) -> Option<Skill>` (lo usa el arnés de evaluación).
- [x] 2.5 Tests de integridad del catálogo: toda tool registrada pertenece al core o a alguna skill; ids únicos; toda skill declara instrucciones y `criteria` no vacías; `clima` cubre `geocode`; `lugares` cubre `geocode` y `reverse_geocode`.

## 3. Selección y ensamblado (dominio, RED→GREEN)

- [x] 3.1 `SkillRouter::select(mensaje, turnos_previos)`: activa la skill cuya probabilidad alcanza `ROUTER_THRESHOLD`; ninguna por encima → selección vacía.
- [x] 3.2 Test de tabla con la matriz de fallo abierto del diseño (apagado, sin key, sin skills enrutables, error, timeout, respuesta ilegible, ids desconocidos, umbral ilegible): todos desembocan en «todas las habilitadas».
- [x] 3.3 `compose_skill_fragments`: solo skills activas, en el orden del catálogo; omite fragmentos en blanco; omite el fragmento cuyo encabezado ya esté en el prompt base.
- [x] 3.4 `exposed_tools(selección, habilitadas)`: `core ∪ seleccionadas ∩ habilitadas`; una tool deshabilitada no se expone nunca, ni aunque su skill fuese seleccionada.

## 4. Cliente de la Decisions API (infra)

- [x] 4.1 Nuevo `src/llm/decisions.rs`: trait `DecisionsProvider` y los tipos de petición/respuesta (`noul`).
- [x] 4.2 `JevDecisionsProvider` con reqwest: `POST {ROUTER_BASE_URL}/alpha/decisions`, cabecera de autorización, timeout propio y modelo del catálogo.
- [x] 4.3 Tests con `wiremock`: forma exacta de la petición (`model`, `state`, `questions.<id>.type=noul`, `instructions`, `criteria`), parseo de `answers` y de `usage.cost`, e ignorado de ids desconocidos.
- [x] 4.4 Tests de error: HTTP 500, timeout, JSON inválido y probabilidad fuera de rango → `Err`, nunca pánico.

## 5. Registry: subconjunto de definiciones

- [x] 5.1 RED: test de `definitions_for(&[&str])` — omite las deshabilitadas, ignora nombres desconocidos y devuelve orden estable por nombre entre llamadas.
- [x] 5.2 GREEN: implementarlo y dejar en verde también el test de caracterización 1.2 (que pasa a describir el camino sin router).

## 6. Orquestador

- [x] 6.1 RED: test de integración con un doble de `DecisionsProvider` — la primera petición al LLM lleva el subconjunto esperado y el clasificador se invoca **una sola vez** aunque el turno tenga tres iteraciones.
- [x] 6.2 GREEN: construir el `SkillRouter` en `Orchestrator::new`, decidir antes del bucle y sustituir `self.registry.definitions()` (`agent.rs:737`) por `definitions_for`.
- [x] 6.3 Tests de fallo abierto de extremo a extremo: provider que falla, umbral no alcanzado y router apagado → las 13 habilitadas.
- [x] 6.4 Test: el mensaje de sistema incluye los fragmentos de las skills activas y no incluye los de las inactivas.

## 7. Configuración

- [x] 7.1 Migración de `settings` con `ROUTER_ENABLED` (false), `ROUTER_MODEL` (`typesafe/jev-1.13`), `ROUTER_THRESHOLD` (`0.3`), `ROUTER_TIMEOUT_MS` (`800`), `ROUTER_HISTORY_TURNS` (`2`) y un `SKILL_<ID>_PROMPT` por skill: idempotente y **sin sobrescribir** valores existentes.
- [x] 7.2 Lectura en caliente en cada turno, con defaults y `warn!` ante valor ilegible.
- [x] 7.3 Test de migración: claves presentes tras migrar y re-ejecución que no pisa un valor editado.

## 8. Arnés de evaluación

- [x] 8.1 Nuevo `src/bin/route-eval.rs` con `--limit`, `--threshold`, `--model` y `--dry-run` (catálogo sin red).
- [x] 8.2 Cobertura por turno (¿está toda tool realmente usada en el conjunto expuesto?) y agregados: activaciones por skill, latencia p50/p95, tokens y coste.
- [x] 8.3 Tests de la parte pura: mapeo `tools_used` → skills y matemática de la cobertura, con casos de tabla (incluido el turno no cubierto).
- [ ] 8.4 Medición real contra la BD de desarrollo y **umbral fijado con los datos**, documentado en el PR.
  - **PENDIENTE**: requiere `OPENROUTER_API_KEY`. El arnés está implementado y verificado en `--dry-run` (66 turnos emparejados en la BD de desarrollo), pero no se ha ejecutado contra el servicio. El enrutador se entrega **apagado**, así que el encendido queda condicionado a esta medición.

## 9. UI

- [x] 9.1 Pestaña «Herramientas»: interruptor del enrutador, umbral (0-1, paso 0,05), modelo de decisiones y mapa de skills con las tools de cada una.
- [x] 9.2 Pestaña «Prompts»: sub-pestaña «Skills» con un área de texto por fragmento; guardar no altera otras claves.
- [x] 9.3 Avisos no bloqueantes: router apagado, umbral 0 o 1.
- [x] 9.4 Tests con `@testing-library/react` (patrón de `SettingsDialog.test.tsx`).

## 10. Cierre

- [x] 10.1 `just check-all` en verde (Rust + frontend).
- [x] 10.2 Review con `@rust-reviewer` y `@react-reviewer`; aplicar hallazgos.
- [x] 10.3 Medición antes/después documentada y decisión razonada de encender o dejar apagado.
- [ ] 10.4 PR a `development` y `openspec archive skill-router`.
