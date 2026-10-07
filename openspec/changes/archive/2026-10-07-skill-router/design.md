# Design

## Context

- El único camino de un turno es `Orchestrator::process_message_stream` (`src/routes/stream.rs:116`), y las herramientas se adjuntan **dentro** del bucle ReAct (`src/orchestrator/agent.rs:714-743`, con `tools: Some(self.registry.definitions())` en la línea 737).
- `ToolRegistry::definitions()` (`src/tools/registry.rs:63`) devuelve todas las habilitadas iterando un `HashMap` (orden no determinista) y filtrando por la tabla `tools`.
- Ya existe un `ContextClassifier` (`src/orchestrator/context_classifier.rs`) que decide **estrategia de contexto** (`!reset`, `!historico`), no herramientas: el nombre nuevo debe ser distinto para no confundir dos clasificadores.
- Los parámetros de generación se leen de `settings` **en cada llamada** (`src/generation.rs`), con `GENERATION_*` sembrados por migración: es el patrón para la configuración del router.
- El endpoint de settings (`src/routes/settings.rs`) acepta claves arbitrarias; el frontend ya tiene el patrón de bloques (`GENERATION_BLOCKS`, `SettingsDialog.tsx:68-72`).
- La Decisions API de Jev es **`alpha`** (`POST {base}/alpha/decisions`), con `state` + `questions` tipadas (`noul`/`choice`/`score`), respuestas en paralelo, `usage.cost`, y salida sin texto libre. `wiremock` ya es dev-dependency.
- El único bloque por capacidad que hoy vive en `settings.system_prompt` es la guía de widgets, añadida **de forma aditiva e idempotente** por `migrations/20261004000001_widget_prompt_guidance.sql` (nunca sobrescribe texto del usuario). Es el molde para los fragmentos.

## Goals / Non-Goals

**Goals:**
- Reducir el bloque de herramientas expuesto por turno sin perder ni una capacidad que el usuario ejerza de verdad.
- Que la decisión sea **una sola** por turno, barata y tipada, con **fallo abierto** garantizado.
- Poder **medir** el umbral con datos reales antes de encender el router, y poder apagarlo en caliente.
- Que un falso negativo sea visible y diagnosticable (log + arnés), y nunca silencioso.

**Non-Goals:**
- **No** se fragmenta ni reescribe el prompt de personalidad: el blob `settings.system_prompt` se queda como está.
- **No** se convierte el router en una frontera de seguridad: permisos y guardrails siguen siendo la capa dura.
- **No** se toca el contrato SSE, el flujo OIDC ni `docker-compose*.yml`.
- **No** se enruta `render_widget` ni `get_current_time` (ver D4).
- **No** se registran las llamadas del router en la tabla de stats (ver D10).
- **No** se expone la API key de OpenRouter en la UI en este cambio.

## Decisions

### D1. Una pregunta `noul` por skill, en una sola petición, una vez por turno
Jev responde varias preguntas independientes **en paralelo** dentro de una misma petición. Se envía una `noul` («¿hace falta esta habilidad?») por cada skill enrutable cuyas herramientas estén habilitadas, con el mensaje actual y los últimos `ROUTER_HISTORY_TURNS` turnos en el `state`.
*Alternativa descartada:* una `choice` con el enum de skills — el `choice` devuelve **una** opción, así que no cubre el turno multiskill («búsqueme una cafetería cerca y créeme un evento mañana»). *Alternativa descartada:* decidir en cada iteración del bucle — multiplicaría el coste y la latencia por turno sin aportar nada, porque el turno se resuelve con el conjunto de skills de entrada.

### D2. Catálogo cerrado con prerrequisitos y pertenencia múltiple
Cada skill declara su conjunto de herramientas **incluyendo los prerrequisitos**: `weather` necesita coordenadas y su propia descripción remite a `geocode`, de modo que la skill `clima` cubre `weather` + `geocode`; `lugares` cubre `search_places` + `geocode` + `reverse_geocode`. Una misma tool puede pertenecer a varias skills; lo que importa es la **unión** del conjunto expuesto. Toda herramienta registrada debe pertenecer al core o a alguna skill (test de integridad), de modo que añadir una tool y olvidarla en el catálogo haga fallar los tests en vez de degradar el enrutado.

### D3. Preguntas dinámicas: solo lo que se puede ofrecer
No se pregunta por una skill cuyas herramientas estén todas deshabilitadas: si el usuario apagó `calendar` en «Herramientas», sobra la pregunta. Menos preguntas, menos tokens y menos superficie de error. Si **ninguna** skill es enrutable, no se llama a Jev.

### D4. El conjunto core (`render_widget`, `get_current_time`) no se enruta
`render_widget` es el esquema **más caro** (3.363 bytes de los ~11,8 KB) y a la vez el peor candidato a ocultar: el prompt base **ordena** usarlo en situaciones concretas (elección entre opciones, recogida de varios datos, listas ejecutables). Ocultarlo dejaría al modelo con una instrucción y sin la herramienta. Se queda en el core y su guía se queda donde está: **no hay que mover contenido del prompt del usuario ni arriesgar una migración destructiva**. `get_current_time` es minúsculo (102 bytes) y lo pide cualquier frase con fecha relativa.
*Alternativa descartada:* enrutar `widgets` y migrar su guía al fragmento — ahorraría 3,4 KB más, pero obliga a **borrar** un bloque del prompt editable del usuario y a aceptar que un falso negativo deje al modelo sin canal de UI. Queda como refinamiento posterior, con el arnés ya en pie.

### D5. La selección vacía es un resultado legítimo
Si ninguna probabilidad alcanza el umbral, el turno es conversacional: se expone **solo el core**. Es el caso «ninguna herramienta» del diagrama, y sale sin necesidad de una opción `None` en el enum: un `noul` no puede inventarse una skill.

### D6. Fallo abierto, con tabla explícita
El router **nunca** puede quitar capacidades por un fallo suyo. La política es una matriz cerrada y testeada, y el resultado del fallo abierto es *exactamente el comportamiento de hoy*:

| Caso | Resultado |
|---|---|
| `ROUTER_ENABLED=false` | Todas las habilitadas; prompt base intacto |
| Sin `OPENROUTER_API_KEY` (modo Ollama) | No se construye el clasificador → todas |
| Ninguna skill enrutable | No se llama a Jev → todas |
| Error HTTP / no-2xx | Todas + `warn!` |
| Timeout (`ROUTER_TIMEOUT_MS`) | Todas + `warn!` |
| Respuesta ilegible o sin `answers` | Todas + `warn!` |
| Ids desconocidos en la respuesta | Se ignoran; el resto se aplica |
| Umbral ilegible en settings | Default (0,3) + `warn!` |

### D7. El filtro es del anuncio, no de la ejecución
`definitions_for` acota lo que se **anuncia**. `ToolRegistry::execute` sigue resolviendo cualquier herramienta habilitada, y `Guardrails` y `Tool::permission` no cambian. Es deliberado: (a) evita romper el turno si el modelo llama a una herramienta no anunciada, y (b) deja claro en la spec que el router **no** es una capa de seguridad.

### D8. Los fragmentos de prompt son aditivos, se omiten si están vacíos y no se duplican
Los fragmentos nacen en claves nuevas (`SKILL_<ID>_PROMPT`), sembradas por migración **idempotente que no sobrescribe**. Se añaden al mensaje de sistema después del prompt base y antes de las secciones compuestas en código (nombre, memoria persistente, episódica, fecha/ubicación), en el orden del catálogo. Reglas de seguridad: un fragmento en blanco no deja rastro; y un fragmento **cuyo encabezado ya esté en el prompt base se omite**, de modo que si el usuario tiene esa guía en su prompt no se duplica nunca. Consecuencia asumida: los fragmentos **suman** tokens, así que su justificación es de **calidad** (guía enfocada por skill), y por eso el arnés mide también con/sin fragmentos.

### D9. La configuración vive en `settings`, se lee en cada turno y arranca apagada
Claves `ROUTER_ENABLED` (`false`), `ROUTER_MODEL` (`typesafe/jev-1.13`), `ROUTER_THRESHOLD` (`0.3`), `ROUTER_TIMEOUT_MS` (`800`), `ROUTER_HISTORY_TURNS` (`2`) y `SKILL_<ID>_PROMPT`. Endpoint y credencial por entorno: `ROUTER_BASE_URL` (por defecto `https://openrouter.ai/api`) y `OPENROUTER_API_KEY` (la misma del chat; en este cambio no se expone en la UI). El modelo **se puede cambiar por UI** porque vive en `settings`, pero el campo se rotula como modelo de **decisiones**: un modelo generativo no habla este endpoint.

### D10. Observabilidad por log, sin tocar `stats`
Se registra con `tracing` la decisión (skills, probabilidades, fuente — router o fallback), la latencia y el `usage.cost`. **No** se escribe en la tabla de stats ni se altera `last_api_call`: la UI de stats sigue midiendo el chat, y contaminarla con el clasificador falsearía la lectura de latencia del modelo principal. El arnés es el instrumento para los números agregados.

### D11. El arnés mide cobertura usando `tools_used` como verdad de referencia
No hace falta etiquetar: cada mensaje de asistente guarda las herramientas que realmente usó (`messages.tools_used`, formato `"(N) tool::operation"`). El arnés recorre los turnos históricos, corre el router sobre el mensaje del usuario y comprueba si **toda** herramienta realmente usada está en el conjunto expuesto → **cobertura**. Con eso se fija el umbral con datos: encender solo si la cobertura es la de hoy (o indistinguible de ella), porque un falso negativo es justo la pérdida que queremos evitar. Limitación asumida y documentada: se compara contra el catálogo de herramientas habilitado **hoy**, no el del momento del turno.

### D12. `definitions_for` con orden determinista
El filtrado por nombre es trivial, pero el orden actual (`HashMap`) no es estable y haría frágiles los tests de la petición. `definitions_for` ordena por nombre, lo que además hace el bloque de herramientas reproducible (mejor para el prompt caching).

## Risks / Trade-offs

- **Endpoint `alpha`**: puede cambiar o desaparecer. Mitigación: trait `DecisionsProvider`, endpoint y modelo configurables, versión pineada y fallo abierto. La feature apagada no tiene coste.
- **Falsos negativos**: el riesgo real de la feature. Mitigación: umbral conservador (0,3), core no enrutable, pertenencia múltiple con prerrequisitos, arnés de cobertura y log de cada decisión.
- **Latencia añadida en el camino crítico**: 50-200 ms según la documentación, con techo duro de `ROUTER_TIMEOUT_MS` (800 ms) y fallo abierto. El router puede apagarse en caliente si el arnés o la telemetría no lo justifican.
- **Los fragmentos suman tokens**: se asumen como coste de calidad; el arnés los mide con y sin.
- **`stats` no cuenta el coste del router**: el gasto del clasificador solo aparece en los logs y en el arnés. Es deliberado (D10) y reversible.
