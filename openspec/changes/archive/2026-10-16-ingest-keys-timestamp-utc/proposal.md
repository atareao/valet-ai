# Proposal: Las fechas de «claves de ingesta» dejan de depender del navegador

## Why

La sección «Claves de ingesta» de la pestaña **API Keys** formatea sus fechas con un parseo
ingenuo:

```ts
const date = new Date(value);
```

pero el backend **no envía ISO-8601**: envía el **formato crudo de SQLite**. Comprobado contra la
base de datos de producción, `ingest_keys.last_used_at` vale:

```
2026-10-12 13:55:33      ← separador de espacio, sin T, sin Z, sin zona
```

En esa misma base **conviven dos convenciones**, y esa mezcla es la razón de que el defecto haya
pasado desapercibido:

| tabla | formato | filas |
|:---|:---|--:|
| `messages` | ISO con `T` | 217/217 |
| `llm_requests` | crudo de SQLite | 523/523 |
| `ingest_keys` | crudo de SQLite | todas |
| `timeline_events` | crudo de SQLite | 1/1 |
| `notifications` | crudo de SQLite | 1/1 |

`new Date()` sobre el crudo de SQLite tiene dos desenlaces, ambos malos y medidos:

- **Safari / WebKit / iOS**: `Invalid Date`. La celda pinta algo degenerado y, como este
  componente devuelve el valor crudo cuando no parsea (`Number.isNaN(...) ? value : ...`), acaba
  mostrando `2026-10-12 13:55:33` en crudo en vez de una ausencia legible. Es un segundo defecto
  del mismo sitio.
- **Chrome / V8**: no falla, pero interpreta el crudo como **hora local** cuando el valor es
  **UTC**, así que muestra la hora desfasada (2 h en Madrid en verano) y **parece correcta**. Este
  es el caso peligroso: un error que no se ve.

Es **el mismo defecto** que se acaba de arreglar en la tarjeta de estadísticas (change
`stats-role-model-view`). Allí se arregló **porque lo vio un revisor, no porque el spec lo
exigiera**. Ese es el motivo de que este change exista aparte: cerrar el defecto en el spec para
que deje de depender de que alguien lo mire, y no dejarlo como un arreglo puntual sin cobertura.

## What Changes

- **MODIFICADO — La lista de claves de ingesta SHALL interpretar `created_at` y `last_used_at`
  como UTC** cuando lleguen en el formato crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`, separador de
  espacio, sin `T` y sin zona). Un valor ya en ISO-8601 se respeta sin tocarlo.
- **MODIFICADO — Un valor ausente o no parseable SHALL mostrarse como una ausencia legible**
  (`—` o equivalente) y **nunca** como el valor crudo ni como «Invalid Date».
- **MODIFICADO — La marca temporal SHALL ser independiente de la zona horaria** de la máquina del
  usuario: el mismo instante no SHALL cambiar según el navegador que lo pinte.

Sin cambios de contrato con el backend: el backend sigue enviando lo que envía; lo que cambia es
cómo lo lee el frontend.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `frontend`: un requisito **modificado** — el de la sección «Claves de ingesta» de la pestaña API
  Keys, que hoy fija el alta y la revocación pero no cómo se leen las fechas que muestra.

## Impact

- **Frontend:** `frontend/src/components/IngestKeysSection.tsx` (la función `formatDate` y su
  manejo de valores no parseables). Es el único punto de cambio de comportamiento.
- **Backend:** ninguno. No se toca Rust.
- **Base de datos:** ninguna migración.
- **Spec:** delta en `frontend` (1 MODIFIED).
- **Decisión abierta (en `design.md`):** dónde vive el helper `parseSqliteTimestamp` que ya
  resuelve esto, hoy colocalizado en `frontend/src/components/stats/sqliteTimestamp.ts`. **Se pide
  decisión al usuario antes de implementar.**
- **Sin cambios** en `docker-compose.prod.yml` ni en el change `stats-role-model-view`.

## Non-goals

- **No se toca el change `stats-role-model-view`.** Está en curso y es de otro agente.
- **No se cambia el backend** ni el contrato de `GET /api/ingest-keys`.
- **No se migran los datos** de la base: la normalización es de lectura, en el frontend.
- **No se toca el resto del componente** (alta, revocación, token de un solo uso): solo la lectura
  de fechas.
- **No se unifican de golpe** las otras vistas que leen el crudo de SQLite (`timeline_events`,
  `notifications`). Este change arregla el componente que se ha verificado; las demás quedan
  registradas como candidatas en el `design.md`.
- **No se toca `docker-compose.prod.yml`.**
