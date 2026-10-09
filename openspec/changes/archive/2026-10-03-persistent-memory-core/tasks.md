# Tasks: Memoria persistente (Capa C) — núcleo

## Bloque 1 — Caracterización y línea base
- [ ] 1.1 Test de caracterización del estado actual: el worker hace una sola extracción (ficha
  episódica, Capa B) y no existe ni la tabla `persistent_memory` ni su inyección. Documentarlo.
- [ ] 1.2 `cargo test` en verde; anotar el baseline (passed/failed).

## Bloque 2 — Migración, configuración y repositorio
- [ ] 2.1 RED/GREEN: migración que crea `persistent_memory` (`id TEXT PRIMARY KEY`,
  `payload TEXT NOT NULL`, `updated_at`) y siembra `settings.consolidator_prompt` y
  `PERSISTENT_MEMORY_BUDGET_TOKENS` (500), con el mismo idioma idempotente que
  `20260929000001_prompts.sql`.
- [ ] 2.2 Verificar que la migración es idempotente y no pisa valores existentes.
- [ ] 2.3 RED/GREEN: repositorio de la Capa C — `get` (sin fila ⇒ vacío, sin error) y
  `upsert` de la única fila `'global_state'`.
- [ ] 2.4 RED/GREEN: `Config::from_env()` lee `SEMANTIC_MODEL` y cae a `MEMORY_MODEL`.

## Bloque 3 — Esquema y marca temporal (funciones puras)
- [ ] 3.1 RED/GREEN: validación de forma y versión (`schema_version = 1`; claves de primer
  nivel permitidas `schema_version`, `user_profile`, `system_rules`; versión distinta ⇒
  rechazo conservando el estado previo; claves desconocidas ⇒ descarte).
- [ ] 3.2 RED/GREEN: sin topes de conteo —ninguna sección se recorta por número—.
- [ ] 3.3 RED/GREEN: `updated_at` escrito por Rust (hash del contenido sin la marca; si no
  cambia se conserva; la fecha del LLM se ignora).

## Bloque 4 — Presupuesto y techo
- [ ] 4.1 RED/GREEN: medición en tokens del JSON minificado contra
  `PERSISTENT_MEMORY_BUDGET_TOKENS`.
- [ ] 4.2 RED/GREEN: compresión única vía LLM cuando se supera el presupuesto, con validación
  del resultado.
- [ ] 4.3 RED/GREEN: si tras la compresión se supera el techo absoluto (doble del presupuesto),
  se rechaza la escritura, se conserva el estado anterior y se registra un aviso.
- [ ] 4.4 Verificar que el estado almacenado es siempre JSON válido y que no se trunca en
  silencio.

## Bloque 5 — Pasada unificada del worker
- [ ] 5.1 RED: un lote produce ficha (B) y estado (C) y marca `is_indexed = 1` en una única
  transacción.
- [ ] 5.2 GREEN: dos extracciones antes de escribir nada; una transacción que escribe
  `memory` + `vec_memory` + `persistent_memory` + la marca.
- [ ] 5.3 RED/GREEN: si falla el consolidador tras una episódica correcta, no se escribe ni la
  ficha ni el estado y no se marca; el reintento produce una única ficha (sin duplicados).
- [ ] 5.4 Cooldown compartido por ambas extracciones; stats de ambas llamadas con `profile_id`
  NULL.
- [ ] 5.5 RED/GREEN: el prompt del consolidador se lee de `settings.consolidator_prompt`, con
  `{{ ESTADO_ACTUAL }}` y `{{ BLOQUE_DE_MENSAJES }}` sustituidos y fallback si falta o está vacío.

## Bloque 6 — Inyección en el mensaje de sistema
- [ ] 6.1 RED: con estado no vacío, la sección persistente aparece entre el prompt y la
  episódica.
- [ ] 6.2 GREEN: rellenar el punto de inserción del mensaje de sistema con el JSON minificado
  más un encabezado corto; leer el estado en cada construcción.
- [ ] 6.3 Verificar que un estado vacío no deja rastro (sin encabezado ni línea en blanco).

## Bloque 7 — Verificación y limpieza
- [ ] 7.1 `cargo fmt --check` limpio.
- [ ] 7.2 `cargo clippy --all-targets -- -D warnings` limpio.
- [ ] 7.3 `cargo test` completo en verde, sin regresiones sobre el baseline.
- [ ] 7.4 `openspec validate persistent-memory-core --strict` válido.
- [ ] 7.5 Anotar en el plan cualquier desviación que revele el código.
