# Proposal: El extractor del timeline no pierde hechos

## Why

El extractor de hechos cronológicos (Capa D) del `EpisodicMemoryWorker` corrió **19 veces** entre el
2026-10-10 y el 2026-10-12 y la tabla `timeline_events` quedó con **un solo hecho** para un día entero
de actividad. Auditado sobre la base de datos de producción:

- **6 de las 19 llamadas terminaron con `error_message = 'NotJson'` y 2 tokens de respuesta.** El
  modelo contesta `[]` (que significa «ningún hecho») y el parser, que exige un **objeto** JSON, lo
  clasifica como fallo.
- **1 llamada terminó con `MissingEvents`** (75 tokens): un objeto sin la clave `events`.
- Las pasadas «buenas» extraen poquísimo: la del mediodía que cubría una carrera cerró con 6 tokens →
  cero hechos.
- Y el lote se marca `is_indexed = 1` pase lo que pase, así que **lo que la extracción no pesca no se
  reintenta: se pierde**. Los usuarios ven un timeline casi vacío.

## What Changes

- **Un array JSON vacío SHALL ser cero hechos, no un error.** Un contenido `[]` SHALL interpretarse
  como «ningún hecho» y SHALL registrarse con `status = 'success'`, no como `NotJson`.
- **Un fallo de forma SHALL reintentarse antes de darse por vencido.** Cuando el contenido no sea un
  objeto JSON interpretable o no contenga la clave `events`, la extracción SHALL repetirse un número
  acotado de veces dentro de la misma pasada; solo tras agotar los intentos SHALL registrar el error y
  continuar sin hechos.
- **El extractor SHALL volver a intentarse con `deepseek/deepseek-v4.1-flash`**, el mismo modelo del
  chat y del consolidador, **con el razonamiento desactivado**. Es un cambio de **configuración**
  (variable `TIMELINE_MODEL` en `.env`, no versionado, y `GENERATION_TIMELINE_REASONING = off`), no de
  código: el requirement «El modelo del extractor SHALL ser configurable y SHALL usar los parámetros de
  su propio rol» ya lo cubre.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `workers`: el requirement del best-effort de la extracción (Capa D) gana la tolerancia del envoltorio
  vacío y el reintento acotado.

## Impact

- **Backend:** `src/workers/episodic_memory.rs` — el parser del envoltorio (`parse_timeline_events` /
  `extract_json_object`) y el punto donde se realiza la extracción del lote.
- **Configuración:** `TIMELINE_MODEL` en `.env` (gitignorado) y `GENERATION_TIMELINE_REASONING` en
  `settings`. **Aviso operativo:** al recrear el contenedor se aplicarán también los valores actuales
  del `.env` para `COLLAPSE_MODEL` y `SEMANTIC_MODEL`, que **hoy no están aplicados en producción** (el
  contenedor corre con valores anteriores). Hay que revisarlos antes de recrear.
- **Spec:** delta MODIFIED en `workers`.

## Non-goals

- **No se cambia la marca de `is_indexed`.** El lote deja de reintentarse (romper el best-effort para
  no perder hechos) se ha **descartado**: obligaría a reprocesar la ficha episódica entera y arriesga
  duplicados. El reintento de la Capa D dentro de la pasada es la alternativa barata.
- **No se toca la validación por hecho** ni el prompt sembrado del extractor.
- **No se añade una UI de timeline.**
- **No se toca `docker-compose.prod.yml`.**
