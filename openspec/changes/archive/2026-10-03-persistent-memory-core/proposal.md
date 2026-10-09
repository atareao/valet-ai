# Proposal: Memoria persistente (Capa C) — núcleo

## Why

Hoy solo hay historial de mensajes (Capa A) y fichas episódicas (Capa B): sucesos con
fecha que se recuperan por similitud. Falta el estado estable —quién es el usuario y qué
reglas ha fijado—, de modo que todo lo que no sea un suceso con fecha se pierde al limpiar
el historial. El mensaje de sistema ya reserva un hueco para esa sección, y
`profiles.preferences` es un JSON que nadie lee. Este change crea la Capa C: un estado JSON
versionado, acotado por presupuesto de tokens y consolidado por el mismo worker que extrae
la episódica, e inyectado como bloque plano.

## What Changes

- **D1 — Tabla `persistent_memory`.** Una única fila lógica (`id = 'global_state'`) con
  `payload` JSON y `updated_at`. Leer sin fila equivale a estado vacío, no a error.
- **D2 — Esquema fijo y versionado.** Claves de primer nivel permitidas
  (`schema_version`, `user_profile`, `system_rules`) y `schema_version = 1`. Versión
  distinta ⇒ se rechaza y se conserva el estado anterior. `user_profile` reúne hechos,
  preferencias e intereses: cambian a ritmos distintos, pero una sola sección evita
  fronteras artificiales. El LLM no puede inventar secciones: el conjunto es fijo.
- **D3 — Sin topes de conteo.** No se limita por número ni el perfil, ni las reglas, ni
  ninguna sub-clave. El único límite de tamaño es el presupuesto de tokens.
- **D4 — La marca temporal la escribe Rust.** Comparación por contenido (hash sin contar la
  fecha): sin cambios, se conserva la fecha; cambiado o nuevo, se sella ahora. La fecha que
  devuelva el LLM se ignora.
- **D5 — Presupuesto y techo.** El estado SHALL caber en
  `PERSISTENT_MEMORY_BUDGET_TOKENS` (def. 500), medido sobre el JSON minificado. Si lo
  supera, una única compresión del LLM. Si tras comprimir sigue por encima del **techo
  absoluto** (el doble, 1000 tokens), Rust **rechaza la escritura y conserva el estado
  anterior**, con un aviso en el log. Si la compresión **falla**, se evalúa el estado sin
  comprimir contra el techo: si cabe, se guarda con un aviso; si no, se conserva el
  anterior. Por tamaño **nunca** se aborta la pasada ni se borra contenido en silencio.
- **D6 — Prompt y modelo configurables.** `settings.consolidator_prompt` (sembrado,
  editable, con fallback) y `SEMANTIC_MODEL` (cae a `MEMORY_MODEL`).
- **D7 — Una sola pasada, una sola marca. BREAKING (interno).** El worker obtiene la
  ficha (B) y el estado consolidado (C) del mismo lote, y **solo entonces** escribe
  ambas y marca los mensajes, en una **única transacción**. `is_indexed = 1` ⇔ B y C
  escritas. Un fallo parcial no deja escritura ni marca (evita fichas duplicadas).
- **D8 — Inyección.** El estado no vacío se inyecta como sección de JSON minificado
  **entre el prompt y la episódica**; vacío no deja rastro.

### Fuera de alcance

- La UI y el endpoint de ver/edición: change aparte (`persistent-memory-ui`).
- Retirar `profiles.preferences`: queda deprecado y sin uso; su limpieza es otra tarea.
- Historial de estados anteriores: se guarda solo la foto actual, no revisiones.

## Capabilities

### New Capabilities

- `persistent-memory`: el estado de la Capa C —su fila JSON versionada, su forma fija, su
  presupuesto y su consolidación—.

### Modified Capabilities

- `workers`: la pasada unificada de la Capa B y la Capa C (dos extracciones, una
  transacción, una marca) y el modelo del consolidador.
- `orchestrator/agent`: la sección de memoria persistente se inyecta en el mensaje de
  sistema; deja de ser un hueco vacío.

## Impact

- **Specs**: `persistent-memory` (nueva, 5 requisitos), `workers` (3 añadidos),
  `orchestrator/agent` (2 modificados, 1 eliminado, 1 añadido).
- **Código**: migración nueva; repo de la Capa C en `src/db/repos/`;
  `src/workers/episodic_memory.rs` (pasada unificada); `src/orchestrator/agent.rs`
  (inyección); `src/config.rs` (`SEMANTIC_MODEL`).
- **Migración**: tabla `persistent_memory` y las claves `consolidator_prompt` y
  `PERSISTENT_MEMORY_BUDGET_TOKENS`.
- **Sin cambios** en `docker-compose.prod.yml` ni en la API pública (la UI es el change
  siguiente).
