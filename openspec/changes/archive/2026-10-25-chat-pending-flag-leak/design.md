# Design: La marca de optimista no debe filtrarse al mensaje canónico

## Context

`useChatSync` es la única fuente de la lista de mensajes del hilo y la mantiene al día con el bus `GET /api/chat/events`. `upsert(message, { pending })` fusiona por id; `sortMessages` ordena por `(created_at, id)` y, además, empuja **al final** todo mensaje cuyo id esté en `pendingIdsRef`.

La marca de optimista vive **fuera** de la lista, en un ref (`pendingIdsRef`) que `upsert` muta. Ese ref y la lista pueden desincronizarse: `upsert` marca el id **antes** de decidir si descarta la escritura, y en la rama de descarte (`return prev`, cuando el id ya está en la lista como fila canónica) **no lo desmarca**. Si la fila canónica del asistente llega por el bus antes que el `done` del stream —el servidor publica el mensaje persistido y luego hace un `await find_by_id` antes de emitir `done`, así que el bus suele ganar—, el id del asistente se queda marcado como pendiente de forma permanente.

Evidencia: la BD de producción es correcta (marcas temporales ascendentes, `user`/`assistant` alternan, un único segundo con dos mensajes y en orden); el log no muestra errores. La secuencia se reprodujo fuera del repositorio con la lógica exacta del hook y produce `u1 u2 a1` donde se espera `u1 a1 u2`.

## Goals / Non-Goals

**Goals**

- Que una fila canónica nunca quede marcada como optimista, con independencia del orden de llegada de las fuentes.
- Que el orden por `(created_at, id)` del servidor prevalezca en toda la lista una vez que existe la fila canónica.
- Eliminar el estado duplicado (ref + lista) que hace posible la desincronización.

**Non-Goals**

- Cambiar el bus de eventos, el cursor o el protocolo de sincronización.
- Tocar el backend, el esquema o la base de datos.
- Persistir el estado de «optimista».
- `docker-compose.prod.yml` ni producción.

## Decisions

### D1 — La marca de optimista viaja en el propio mensaje, no en un ref aparte

`Message` (tipo interno del cliente) SHALL admitir un campo opcional `pending?: boolean` que SHALL NOT enviarse nunca al servidor y SHALL NOT persistirse. `sortMessages` SHALL leer ese campo del propio mensaje, no un `Set` de identificadores. Como la marca está unida a la fila que se inserta o sustituye, ninguna fila canónica puede heredarla: al fusionar la fila del servidor (que no trae la marca), la fusión la **retira**; y al fusionar una escritura optimista sobre una fila ya canónica, la escritura se descarta (D2).

### D2 — La guarda «una escritura optimista no pisa una fila canónica» se mantiene

Hoy, cuando el id ya existe como fila canónica, `upsert` hace `return prev` para no pisar la fila del servidor con la versión local (marca temporal del reloj del navegador y contenido parcial). Esa guarda SHALL conservarse, pero trasladada a la fusión pura: si el mensaje entrante está marcado como pendiente y ya existe una fila **canónica** (sin la marca) para ese id, la fusión SHALL descartar la escritura entrante. Como la marca viaja en el mensaje y no en un estado paralelo, **la escritura descartada no deja ningún residuo**: ese era el bug.

### D3 — La marca se retira al fusionar la fila canónica

Cuando se fusiona una fila canónica (sin la marca) sobre una fila optimista existente, la fusión SHALL retirar la marca del resultado, de modo que la fila vuelva a su posición dictada por `(created_at, id)`. Al fusionar una escritura optimista (`pending: true`) sobre otra fila optimista, la marca se conserva.

### D4 — Se elimina `pendingIdsRef`

`upsert` SHALL dejar de mantener un estado paralelo. La firma `upsert(message, { pending })` SHALL seguir existiendo (quien propone la burbuja optimista sigue indicando que lo es), pero la marca se resuelve **dentro del mismo upsert**: se adjunta al objeto que se fusiona. El bus y el `done` canónicos llaman sin `pending` y por tanto producen filas sin marca. `sortMessages`, `upsertMessage` y `upsertMessages` SHALL dejar de recibir un `Set` de identificadores pendientes.

### D5 — Test de regresión de la carrera

SHALL añadirse un test que reproduzca la secuencia exacta: burbuja optimista de usuario → fila canónica de usuario por el bus → fila canónica de asistente por el bus → `done` del stream con el mismo id del asistente → envío de un mensaje nuevo. El resultado SHALL ser que el asistente anterior permanece por encima del mensaje nuevo, y que ningún id queda marcado como pendiente tras el `done`.

## Risks / Trade-offs

- La guarda de D2 sigue descartando la escritura optimista cuando la fila canónica ya está: el texto parcial optimista del `done` no se pinta si el bus llegó antes. Es el comportamiento correcto (la fila canónica es la verdad) y el que ya existía; lo que cambia es que deja de corromper el orden.
- El campo `pending` es un detalle interno del cliente: si algún día un mensaje se serializara hacia el servidor, habría que omitirlo explícitamente. Se documenta en el tipo.
