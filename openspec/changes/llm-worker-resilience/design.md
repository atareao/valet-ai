# Design: Resiliencia de los workers ante respuestas LLM inservibles

## Context

La memoria episódica de Valet se escribe en tres capas encadenadas dentro del `EpisodicMemoryWorker`: la **Capa A** extrae hechos episódicos del lote de mensajes, la **Capa B** escribe la ficha del lote (`is_indexed = 1`), y la **Capa C** consolida el estado semántico acumulado. La Capa C depende de la B y hoy, si falla, mata toda la pasada. El 2026-10-10, un modelo que razona agotó el `max_tokens` con tokens de razonamiento, el consolidador devolvió vacío en los dos intentos, y el lote quedó bloqueado reintentándose cada 30 minutos sin avanzar ni delatarse (la respuesta vacía se guardaba como `status='success'`).

## Goals / Non-Goals

**Goals**

- Que un fallo del consolidador o del colapso por contenido vacío, truncado o JSON inválido nunca sea silencioso.
- Que un truncamiento por presupuesto se reintente con más presupuesto en vez de repetir la misma tirada.
- Que un fallo de la Capa C degrade la pasada en vez de abortarla, conservando el estado semántico anterior.
- Que el diagnóstico quede registrado en `llm_requests` con `status='error'`.

**Non-Goals**

- No se tocan los prompts ni se añaden ajustes nuevos.
- No se toca la UI.
- No se corrige la deriva de `docker-compose.prod.yml`.

## Decisions

### D1 — Degradar en vez de abortar

Hoy un fallo de la Capa C mata la Capa B. Se cambia: un fallo de la consolidación escribe la ficha y la marca, y conserva el estado anterior. El invariante «`is_indexed = 1` ⇒ existe un estado de Capa C válido» se mantiene —el anterior—. El motivo es que perder la ficha y bloquear el lote para siempre es peor que un perfil que se actualiza una pasada más tarde. Un fallo de la **extracción episódica** mantiene el comportamiento estricto actual: nada escrito, nada marcado.

### D2 — Escalar el presupuesto en vez de un tope de razonamiento explícito

Se evaluó enviar un tope de razonamiento propio (`reasoning.max_tokens`, específico de OpenRouter) y se descarta: los modelos que incumplen el flag `enabled:false` no ofrecen garantía de respetarlo. En su lugar la garantía es **sobre el resultado**: si `finish_reason = length`, se reintenta una vez con el **doble** de presupuesto; si vuelve a fallar, se degrada (D1) y se registra el error. Es agnóstico del proveedor y del modelo.

### D3 — `finish_reason` tipado

`ChatResponse` gana `finish_reason: Option<FinishReason>` con `FinishReason::{Stop, Length, Other(String)}`; `None` cuando el proveedor no lo envía (p. ej. Ollama). Se prefiere un enum a un `String` para que el worker no compare cadenas mágicas.

### D4 — Un cuerpo sin `choices` es un error

`parse_response` rechazará el cuerpo (200 con `{"error": …}`) en vez de devolver contenido vacío como éxito.

### D5 — Guarda del colapso

Un resumen vacío o solo-espacios no se escribe nunca en `messages.collapsed_content`; se registra como error y el mensaje queda sin colapsar. El motivo es que hoy se escribe `''` y se marca como colapsado, lo que es **pérdida silenciosa de contenido** que nadie delata —a diferencia del consolidador, que al menos para la memoria.

### D6 — Modelo de los roles mecánicos: decisión de configuración, no de código

Se documenta el bake-off y se fija en las plantillas del repo: `SEMANTIC_MODEL=qwen/qwen3-235b-a22b-2507`, `COLLAPSE_MODEL=mistralai/mistral-small-24b-instruct-2501`, `MEMORY_MODEL=mistralai/mistral-small-24b-instruct-2501`. El id exacto importa: sin el sufijo `-2507`, `qwen3-235b-a22b` es la variante pensante.

## Risks / Trade-offs

- La degradación puede dejar el perfil desactualizado una pasada.
- El escalado duplica el coste de una llamada fallida.
- El bake-off es n=15 por modelo y sobre un único estado de partida: la elección de modelo es revisable, no un contrato.

## Migration Plan

Sin migraciones de datos ni de esquema. El cambio es de código y de plantillas de entorno; se despliega con el build habitual. Las instancias existentes que sigan apuntando a un modelo razonador seguirán funcionando, solo que ahora el fallo será visible y degradado en vez de silencioso y bloqueante.

## Open Questions

- ¿Conviene reintentar más de una vez con presupuesto creciente, o una sola vez basta?
- ¿La guarda del colapso debería también descartar resúmenes degenerados por longitud mínima, además de los vacíos?
