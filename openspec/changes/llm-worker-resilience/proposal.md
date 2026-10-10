# Change: Resiliencia de los workers ante respuestas LLM inservibles

## Why

El 2026-10-10 la memoria episódica dejó de avanzar en producción. El log:

```
WARN initial consolidation produced no valid state; retrying once attempt=1 content_len=0
ERROR EpisodicMemoryWorker: state consolidation failed; aborting the pass error=consolidator returned an invalid state: consolidator returned no JSON object (content_len=0, preview="")
```

El consolidador devolvió **contenido vacío en los dos intentos** y la pasada se abortó. Tres causas encadenadas:

1. `SEMANTIC_MODEL` apuntaba a `deepseek/deepseek-v4.1-flash`, un modelo **que razona**: ignora el `reasoning: {"enabled": false}` que envía Valet y sus tokens de razonamiento se descuentan del `max_tokens`. Reproducido con la petición real: 5 tiradas → un JSON cortado por el techo (`finish=length`, `completion=2048`, `reasoning=1693`) y una respuesta de **3 caracteres**.
2. El reintento de la consolidación usa **los mismos parámetros** (lo exige la spec), así que repite la misma tirada.
3. Al fallar la consolidación **se aborta la pasada entera**: no se escribe la ficha ni la marca, el lote queda sin indexar y se reintenta cada 30 minutos **indefinidamente** — la memoria no se degrada, se para.

Y el fallo fue **invisible**: una respuesta vacía se registra como `status='success'` en `llm_requests`, `ChatResponse` no expone `finish_reason`, y un cuerpo HTTP 200 con `{"error": …}` (sin `choices`) se acepta como respuesta válida vacía.

Mitigación aplicada por configuración (fuera de este change): bake-off de 5 lotes reales × 3 tiradas × 4 modelos sobre el prompt y el estado reales:

| modelo | JSON inválido | truncado | razonamiento | pérdidas de datos del estado | ~$/mes |
|:--|:--:|:--:|:--:|:--:|--:|
| `mistral-small-24b-instruct-2501` | 0/15 | 0 | 0 | 20 | $0,21 |
| `mistralai/mistral-nemo` | 3/15 | 0 | 0 | 83 | $0,09 |
| `qwen/qwen3-30b-a3b-instruct-2507` | 0/15 | 0 | 0 | 15 | $0,55 |
| `qwen/qwen3-235b-a22b-2507` | 0/15 | 0 | 0 | **0** | $0,68 |
| `deepseek/deepseek-v4.1-flash` (lo que había) | — | — | se come el presupuesto | — | ~$4,60 |

## What Changes

- `ChatResponse` SHALL exponer el **motivo de finalización** (`finish_reason`), para distinguir una respuesta truncada de una completa.
- Una respuesta HTTP 200 **sin `choices`** (p. ej. `{"error": …}` del proveedor) SHALL ser un error, no una respuesta vacía.
- El reintento de la consolidación SHALL usar **presupuesto ampliado** cuando la causa sea un corte por presupuesto, en vez de repetir la misma tirada.
- Un fallo de la consolidación SHALL **degradar** la pasada (se escribe la ficha y la marca, se conserva el estado anterior) en vez de abortarla.
- Un fallo del consolidador SHALL registrarse como fila `status='error'` en `llm_requests` con el diagnóstico (motivo de finalización, uso y preview).
- El `CollapseWorker` SHALL NOT escribir un resumen **vacío o degenerado**: lo registrará como error y dejará el mensaje sin colapsar.
- Las plantillas de entorno del repo (`.env.j2`, `.env.example`) SHALL NOT sembrar modelos que razonan en los roles mecánicos.

## Impact

- Afecta: `src/llm/provider.rs`, `src/llm/openrouter.rs`, `src/workers/episodic_memory.rs`, `src/workers/collapse.rs`, `.env.j2`, `.env.example`.
- Specs: `workers` (5 modificados/eliminados, 4 añadidos), `llm/provider` (1 añadido), `llm/openrouter` (1 añadido).
- Sin migraciones, sin claves nuevas en `settings`, sin cambios de UI.

## Non-goals

- NO se toca `docker-compose.prod.yml` ni la deriva entre el compose del repo y el despliegue real de producción (el contenedor de prod se llama `valet` y sí recibe las variables de modelo; el fichero del repo fija `valet_valet` y no las reenvía). Queda registrado como hallazgo aparte.
- NO se cambian los prompts (`archivist_prompt`, `consolidator_prompt`) ni se añaden ajustes nuevos.
- NO se toca la UI.
