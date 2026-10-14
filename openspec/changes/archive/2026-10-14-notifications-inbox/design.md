# Design

## Evidencia

**1. El bucle de `mark_read` sin `ids`.** Turnos del **2026-10-13 19:12** y **2026-10-14 05:04**.
Log del contenedor:

```
list_unread                                    → OK (devuelve 3 avisos)
mark_read  tool_args={"operation":"mark_read"}  ← `ids` ausente
  → Invalid arguments: se requiere al menos un `id` en `ids`
  → (6–7 reintentos con el mismo `tool_args`)
  → ⚠️ Tool retry limit reached tool_name=notifications::mark_read
```

Estado de la base tras esos turnos: los **3 avisos siguen con `read_at = NULL`**. El modelo llegó a
afirmar que había pasado los ids; falso: ningún reintento incluyó `ids`.

Lo que esto **descarta** como diagnóstico:

- **La herramienta no está rota.** `list_unread` responde OK y tabula los avisos.
- **El agente no está ciego.** Ve los avisos; lo que no puede es expresar «todas».
- **No es un fallo de permisos** (`NoConfirm` permite la operación) ni del registro de herramientas.

La causa es de contrato: la operación «marcar todas las no leídas» **no existe**. `mark_read` exige
`ids` no vacío —en `src/tools/notifications.rs` rechaza con `ToolError::InvalidArguments("se
requiere al menos un `id` en `ids`")`, y `POST /api/notifications/mark-read` responde `400` con
`ids` ausente o vacío—. Un modelo que quiere «todas» no tiene forma de decirlo.

**2. Cero UI de bandeja.** No hay consumidores de `/api/notifications/*` en el front; el único
punto que toca notificaciones es el tab de suscripción push de los ajustes.

**3. `list_unread` ya aceptaba offset, pero la ruta lo fijaba a 0.** El repositorio firma
`list_unread(pool, limit, offset)` y ordena `created_at DESC, rowid DESC`, pero la ruta llama
`list_unread(&state.db, limit, 0)` y responde siempre `next_cursor: None`. La paginación estaba a
medio construir y nunca se ejercitó. La bandeja nueva la activa.

**4. El crudo de SQLite, otra vez.** `notifications.created_at` es `datetime('now')` crudo
(`"YYYY-MM-DD HH:MM:SS"`, sin `T`, sin zona), la **misma convención** que ya causó el defecto de
fechas en este repo:

| navegador | `new Date("2026-10-14 05:04:00")` |
|:---|:---|
| Safari / WebKit / iOS | `Invalid Date` |
| Chrome / V8 | lo lee como hora **local**; el valor es **UTC** → desfase que **parece correcto** |

El diálogo no puede repetir ese parseo ingenuo.

---

## D1 — «Marcar todo»: `ids` ausente, array vacío explícito sigue siendo error

En `mark_read`, el campo `ids` **ausente** marca **todas** las no leídas. Un `ids: []` **explícito**
sigue siendo error (`InvalidArguments` en la herramienta, `400` en la ruta).

El razonamiento es de intención: la **ausencia** de `ids` es una petición deliberada de «todas»; un
array **vacío** es una petición malformada (el llamante quiso pasar ids y no pasó ninguno). Tratar
ambos casos igual borraría esa distinción y haría de `ids: []` un «marca todo» silencioso, que no es
lo que el llamante pidió.

**Alternativa descartada:** un flag `all: true`. Añade un modo y un campo, y deja `ids` y `all` como
estados mutuamente excluyentes que hay que validar (¿`all: true` con `ids` no vacío? ¿error?).
Omitir `ids` no añade superficie: el contrato ya tiene el campo y su ausencia ya es significativa.

## D2 — Paridad ruta / herramienta

`POST /api/notifications/mark-read` y `notifications.mark_read` comparten **la misma semántica**,
incluida la marca masiva: ambas omiten `ids` para marcar todo y ambas rechazan `ids: []`. Es la
regla que ya rige el resto de la capability (la herramienta delega en el mismo repositorio). La UI
del diálogo usa la **ruta** para su botón «Marcar todos»; el **agente** usa la **herramienta**. Si
las dos divergieran, el bug volvería por el otro lado.

## D3 — Paginación por cursor keyset, no offset

La bandeja ordena `created_at DESC, rowid DESC` porque `created_at` es `datetime('now')` con
**granularidad de segundo**: dos avisos del mismo segundo empatan y, sin `rowid`, el orden sería no
determinista. Por eso el cursor **codifica ambos campos**, `"<created_at>|<rowid>"`, y la consulta
usa:

```sql
WHERE read_at IS NULL
  AND (created_at < ?1 OR (created_at = ?1 AND rowid < ?2))
ORDER BY created_at DESC, rowid DESC
LIMIT ?3
```

Un cursor con **solo** `created_at` **se saltaría** los avisos empatados con el último de la página:
al avanzar a `created_at < ?`, todos los del mismo segundo que el frontera desaparecen aunque no se
hayan mostrado. Con `rowid` como desempate, la frontera es total y no se pierde ninguna fila.

**El offset se descarta** además por un motivo independiente: marcar como leído **elimina filas** de
la lista de no leídas, así que un `OFFSET n` calculado sobre la página anterior apunta a un
conjunto que ya no es el mismo. Un keyset sobre el último elemento mostrado es estable aunque el
conjunto mengüe por delante.

## D4 — Tamaño de página: el 10 es del consumidor

El diálogo pide `limit=10`. La ruta **conserva** su default (50) y su máximo (200); no se recorta el
contrato por la UI. El 10 es elección del consumidor (el hook `useNotifications(pageSize = 10)`), no
del backend.

## D5 — Navegación por páginas con pila de cursores

«Anterior» / «Siguiente» con una **pila de cursores** (la frontera de cada página), no scroll
infinito ni «cargar más». Coherente con «máximo de 10 y paginado», y compatible con mostrar el
recuento con `total` («N avisos pendientes», que es global, no de la página). `goNext` empuja el
`next_cursor` recibido y avanza; `goPrev` retrocede en la pila. La pila existe porque el keyset es
unidireccional: para volver atrás hay que recordar los cursores ya vistos.

## D6 — Forma del diálogo: espejo de `TaskView`

`NotificationsView` se carga con `React.lazy` dentro de un `Modal` de `AppLayout`
(`title="🔔 Avisos"`, `width` 1000, `footer={null}`), abierto por un botón `BellOutlined` en el
header, junto a los de Tasks / Agenda / Stats. Es el molde exacto de `TaskView`. **Sin pestaña
nueva de ajustes**: la bandeja es una vista operativa, no una preferencia.

## D7 — El diálogo lleva las dos acciones

«Marcar como leído» **por fila** y «Marcar todos como leídos» **global**. Un aviso suelto se marca
con la primera; la limpieza tras revisar se hace con la segunda. La ruta soporta ambas (D1/D2).

## D8 — El aviso es crudo de SQLite: se interpreta como UTC

`notifications.created_at` llega como `datetime('now')` crudo (`"YYYY-MM-DD HH:MM:SS"`, sin `T`,
sin zona). Renderizarlo con `new Date()` reproduce el defecto conocido (evidencia 4): WebKit lo
rechaza (`Invalid Date`) y V8 lo lee como hora **local** cuando es **UTC**. El diálogo **SHALL**
interpretar `created_at` como **UTC**, mostrarlo en la hora **local** del navegador dentro de un
`<time dateTime="<ISO UTC>">` (el `dateTime` expone el instante UTC) y mostrar `—` cuando no sea
parseable, **nunca** el valor crudo ni «Invalid Date». Se usa el helper existente
`parseSqliteTimestamp` (reescribe el separador
a `T`, añade `Z`, respeta ISO-8601 con zona y devuelve `null` si no parsea). El diálogo **no** pinta
`metadata`: no aporta al lector y solo añade ruido.

## D9 — Dónde vive `parseSqliteTimestamp` — **RESUELTA**

El helper `parseSqliteTimestamp` **se mueve** a `frontend/src/utils/sqliteTimestamp.ts` (fichero
nuevo, hogar compartido). El movimiento incluye **test propio** para el helper, y el import de
`frontend/src/components/stats/BackgroundCard.tsx` y
`frontend/src/components/stats/BackgroundCard.test.tsx` se **reapunta** al nuevo fichero. El fichero
viejo `frontend/src/components/stats/sqliteTimestamp.ts` **desaparece**.

Antes del movimiento, el helper estaba en `frontend/src/components/stats/sqliteTimestamp.ts` y **no
tenía test propio**: sus 3 aserciones vivían dentro de `BackgroundCard.test.tsx`, que lo importaba.
El repo **no tenía `src/utils/` ni `src/lib/`**: el patrón previo era el `.ts` colocalizado junto al
componente que lo usaba. La prueba de no-regresión del movimiento es que la suite de stats siga verde
tras reapuntar el import.

**Alternativas descartadas:**

- **Duplicar el helper** junto a `NotificationsView`: las dos copias **divergen**; es justo el
  patrón que causó el problema (el arreglo de stats no llegó a ingesta).
- **Importar desde `components/stats/`**: acopla un componente **no-stats** a una carpeta que nació
  dentro del change de estadísticas; deja la utilidad mal ubicada de forma permanente.

Esta decisión cierra además de facto la **decisión abierta D2 del change hermano
`ingest-keys-timestamp-utc`**, que pedía un hogar compartido para el mismo helper: ese hogar ya
existe y es `frontend/src/utils/sqliteTimestamp.ts`.

---

## No-goals

- No se toca el contrato de ingesta (`POST /api/notifications`).
- No se añade una pestaña de ajustes para la bandeja.
- No se cambia el comportamiento de `notificationclick` del service worker.
- No se arreglan de golpe las demás vistas que leen el crudo de SQLite (D8 aplica al diálogo).
- No se migran datos.
- No se toca `docker-compose.prod.yml`.
- No se implementa `ingest-keys-timestamp-utc`, aunque D9 le resuelva la ubicación del helper.
