# Proposal: Reagrupar el prompt en un único mensaje de sistema

## Why

La petición al LLM abre hoy con **tres** mensajes `role:"system"` separados: el
prompt, la memoria episódica y la fecha, hora y ubicación.

| orden | mensaje `role:"system"` |
|---|---|
| 1 | `settings.system_prompt` |
| 2 | sección de memoria episódica (si hay fichas) |
| 3 | fecha, hora y ubicación (si el navegador la manda) |

Tener la instrucción partida en varios mensajes de sistema no aporta nada al
modelo y complica razonar sobre el prompt: cada sección vive en un punto distinto
y su orden no está garantizado en un solo sitio. Reagruparlo en un único mensaje:

- Deja un **orden explícito** y de lo estable a lo volátil: prompt → (futura)
  persistente → episódica → «ahora».
- Pone la fecha, hora y ubicación **al final**, pegadas al turno, que es donde más
  pesan.
- **Prepara el hueco** de la memoria persistente: la feature siguiente solo rellena
  una sección, sin reordenar el mensaje.
- Da **un único punto de ensamblado** y testeable, ahora que solo hay una ruta.

Ya no hay dos rutas: el change anterior retiró la entrada sin streaming, así que
todo esto vive en `process_message_stream`.

## What Changes

- **D1 — Un único mensaje de sistema.** La petición SHALL abrir con un solo mensaje
  `role:"system"` que reúna, en este orden y **solo si existen**, las secciones:
  prompt (`settings.system_prompt`) → hueco reservado de memoria persistente →
  memoria episódica → fecha, hora y ubicación. Va antes del historial.
- **D2 — La fecha, hora y ubicación cierran el mensaje.** Pasan a ser la **última**
  sección, con el formato actual (`{fecha}.` y, si hay, `Ubicación: {nombre}
  ({lat}, {lon}).` o `Coordenadas: ({lat}, {lon}).`). No lleva encabezado propio: el
  texto ya se entiende y ahorra tokens. Se omiten por completo si no hay contexto de
  navegador.
- **D3 — Hueco reservado, sin texto.** La posición de la memoria persistente SHALL
  quedar reservada en el constructor (parámetro opcional, hoy siempre vacío). **No**
  se inyecta título, marcador, comentario ni línea en blanco: cero tokens de
  relleno hasta que exista la feature.
- **D4 — Secciones vacías, omitidas.** Ninguna sección ausente deja rastro: sin
  títulos huérfanos ni separadores. Es el comportamiento que ya tiene la episódica,
  extendido a las demás.
- **D5 — Se conserva lo que ya funciona.** Siguen intactos la etiqueta
  `<episodic_memory>`, su título `# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)` y su
  instrucción.

### Fuera de alcance

- El **contenido** de la memoria persistente (tabla, extracción, edición): es una
  feature aparte. Aquí solo se reserva el hueco.
- Retirar `BuiltContext.system_prompt`, campo **vestigial** que el propio código
  documenta como muerto: limpieza ajena a este cambio.

## Impact

- **Specs**: `orchestrator/agent` — 3 requisitos añadidos, 2 modificados.
- **Código**: el ensamblado del prompt en `src/orchestrator/agent.rs`.
- **Sin migración** y sin cambios de esquema.
