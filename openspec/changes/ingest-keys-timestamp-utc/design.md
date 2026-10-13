# Design

## Evidencia

Todo lo que sigue sale de la **base de datos de producción**. La consulta de contraste es:

```sql
SELECT 'ingest_keys' AS tabla, last_used_at FROM ingest_keys LIMIT 1;
-- 2026-10-12 13:55:33     ← separador de espacio, sin T, sin Z, sin zona
```

Y la mezcla de convenciones que explica por qué el defecto pasó desapercibido:

| tabla | formato | filas |
|:---|:---|--:|
| `messages` | ISO con `T` | 217/217 |
| `llm_requests` | crudo de SQLite | 523/523 |
| `ingest_keys` | crudo de SQLite | todas |
| `timeline_events` | crudo de SQLite | 1/1 |
| `notifications` | crudo de SQLite | 1/1 |

`messages` —la tabla que más se mira, la del chat— usa ISO con `T`, y ahí `new Date()` funciona.
Las tablas que usan `datetime('now')` crudo son minoría y no están todas en pantalla, así que el
defecto vive en los bordes y solo se ve si alguien mira con el navegador equivocado.

**Consecuencias del parseo ingenuo, medidas:**

- **Safari / WebKit / iOS** → `Invalid Date`. Como el componente devuelve el valor crudo cuando no
  parsea (`Number.isNaN(date.getTime()) ? value : date.toLocaleString()`), la celda acaba
  mostrando `2026-10-12 13:55:33` en crudo: no una ausencia legible, sino el propio defecto
  disfrazado de dato.
- **Chrome / V8** → no falla, pero lee el crudo como **hora local** cuando es **UTC**. En Madrid en
  verano eso son **2 h de desfase** que **parecen correctas**. Es el peor caso: el error que no se
  ve.

Es **el mismo defecto** ya arreglado en la tarjeta de estadísticas (`stats-role-model-view`), donde
se arregló **porque lo vio un revisor, no porque el spec lo exigiera**. Este change existe para
fijarlo en el spec y que deje de depender de la suerte.

---

## D1 — La normalización: `parseSqliteTimestamp`

El helper que resuelve esto ya existe y está probado:

`frontend/src/components/stats/sqliteTimestamp.ts`

```ts
const SQLITE_TIMESTAMP_RE = /^(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}:\d{2}(?:\.\d+)?)$/;

export const parseSqliteTimestamp = (value: string | null): Date | null => {
  if (!value) return null;
  const match = SQLITE_TIMESTAMP_RE.exec(value);
  const normalized = match ? `${match[1]}T${match[2]}Z` : value;
  const dt = new Date(normalized);
  return Number.isNaN(dt.getTime()) ? null : dt;
};
```

Hace exactamente lo que el requisito pide: reescribe el separador a `T` y añade `Z` para fijar
UTC; respeta el ISO-8601 que ya traiga zona; y devuelve `null` cuando no es parseable. **No se
reinventa.** El trabajo de este change es aplicarlo en `IngestKeysSection.tsx` y decidir **dónde
vive**.

### El formato del componente

`formatDate` pasa de devolver el valor crudo cuando falla a devolver `—`:

```ts
function formatDate(value: string | null): string {
  const date = parseSqliteTimestamp(value);
  return date ? date.toLocaleString() : "—";
}
```

La regla del requisito —«ausencia legible, nunca el valor crudo ni "Invalid Date"»— se cumple
porque el `null` del helper y el `null` de la fila colapsan en el mismo `—`, que es justo lo que
se quiere: para el lector, «no hay fecha» y «no se pudo leer» son la misma cosa.

---

## D2 — Dónde vive el helper: **decisión abierta, se pide al usuario**

El helper nació **dentro del change de estadísticas** y vive en
`frontend/src/components/stats/sqliteTimestamp.ts`. Importarlo desde `IngestKeysSection.tsx` es
posible hoy, pero deja el módulo en un sitio discutible: una utilidad de fechas colgando de
`components/stats/`, que no es un módulo compartido.

Este repo **no tiene `src/utils/` ni `src/lib/`**: el patrón es el `.ts` colocalizado junto al
componente que lo usa. No hay un hogar «compartido» evidente al que moverlo, y crearlo es una
decisión de estructura, no una consecuencia mecánica del arreglo. Por eso **no se resuelve aquí**:
se plantean las opciones y se pide decisión.

**Opción 1 — Mover a un módulo compartido.**
`frontend/src/utils/sqliteTimestamp.ts` (o `src/lib/…`), importado por `stats/` y por
`IngestKeysSection`.
- *Coste:* introduce una **convención de directorio nueva** que hoy no existe, y toca el change de
  estadísticas (que está en curso) para reapuntar su import. Un cambio de infraestructura pequeña
  metido en un arreglo de fechas.
- *Ganancia:* un único hogar para la utilidad; el próximo consumidor la encuentra donde la busca.

**Opción 2 — Duplicar el helper** junto a `IngestKeysSection` (p. ej.
`frontend/src/components/IngestKeysSection.ts` o una copia colocalizada).
- *Coste:* dos copias que **divergen** con el tiempo; el mismo defecto se arregla dos veces. Es
  exactamente el patrón que ha causado esta situación (el arreglo de stats no llegó a ingesta).
- *Ganancia:* cero acoplamiento con el change de estadísticas; se puede hacer sin tocar nada de
  otro agente.

**Opción 3 — Importar desde `stats/`.**
`import { parseSqliteTimestamp } from "./stats/sqliteTimestamp"`.
- *Coste:* acopla un componente **no-stats** a un módulo nacido dentro del change de estadísticas;
  si ese change reorganiza o renombra el fichero, este componente se rompe sin relación aparente.
  Deja la utilidad mal ubicada de forma permanente.
- *Ganancia:* es el camino de **mínimo cambio** y no duplica lógica.

**Recomendación (no vinculante):** la Opción 1, pero **solo si el usuario acepta** crear el
directorio compartido. Si se prefiere no tocar nada del change de estadísticas mientras esté
abierto, la Opción 3 es un puente aceptable y la Opción 2 la peor de las tres.

> **Se pide decisión explícita antes de implementar.** El `tasks.md` deja esta decisión como
> bloqueante del primer paso de GREEN.

### Otros consumidores que podrían necesitarlo mañana

El crudo de SQLite no es exclusivo de esta tabla. Cualquier vista que lea `llm_requests`,
`timeline_events` o `notifications` con `new Date()` tendrá **el mismo defecto**. En concreto,
`timeline_events` y `notifications` ya están en la base con formato crudo; si se pintan en pantalla
con un parseo ingenuo, arrastran el bug. No se arreglan en este change (ver Non-goals), pero
refuerzan el argumento de que el helper merece un **hogar compartido** (Opción 1) en vez de vivir
colgado de `stats/`.

---

## Decisión consciente: se arregla el síntoma visible, no toda la clase

Este change arregla **el componente verificado** (`IngestKeysSection`), no todas las vistas que
leen el crudo de SQLite. Es deliberado:

- **Qué se gana.** Se cierra el defecto medido, con escenarios en el spec, y se deja constancia de
  que la clase de defecto es más amplia (la tabla de convenciones de arriba es la prueba).
- **Qué se deja fuera.** `timeline_events` y `notifications` pueden tener el mismo problema. No se
  tocan aquí porque **no se han verificado en pantalla** y porque ampliar el alcance a vistas no
  comprobadas es justo lo que convirtió el arreglo de stats en un parche invisible.
- **Qué queda en pie.** El helper existe y funciona; el día que otra vista lo necesite, la decisión
  de dónde vive (D2) ya estará tomada.

---

## No-goals

- No se toca el change `stats-role-model-view` (en curso, de otro agente).
- No se cambia el backend ni el contrato de `GET /api/ingest-keys`.
- No se migran datos: la normalización es de lectura.
- No se arreglan de golpe las demás vistas que leen el crudo de SQLite (D2 las deja registradas).
- No se toca `docker-compose.prod.yml`.
