# Design

## Token por aplicación, no una clave única compartida

Varias aplicaciones van a enviar avisos: un formulario de WordPress, Uptime Kuma, scripts propios y
workflows de GitHub Actions. Con una **clave única compartida**, comprometer una obliga a
reconfigurar **todas** a la vez, y no hay forma de saber qué aplicación envió qué. Una clave **por
aplicación** permite revocar Uptime Kuma sin tocar WordPress, y `last_used_at` responde a la pregunta
operativa «¿esta app sigue enviando?». El coste es una tabla con nombre por clave, que es barato.

## Solo el SHA-256, y el token se muestra una vez

El token se genera aleatorio de **256 bits**, así que no hay diccionario que atacar y un KDF lento
(`argon2`) no aporta nada a la búsqueda por fuerza bruta, mientras encarece cada petición de ingesta.
Se guarda únicamente el **SHA-256** del token. Alternativas descartadas: guardarlo en claro (una copia
de la base de datos entregaría las llaves de la puerta, igual que entregaría un endpoint de push) y
permitir releerlo después (no hay nada que leer: solo hay hash). El token se muestra **una sola vez**,
en la respuesta de alta.

## Tabla propia y no `settings`

Las claves no son un ajuste único: son una **colección** con nombre, alta, baja, revocación
individual y `last_used_at`. Eso no cabe en la tabla `settings`, que es clave-valor de valor único.
Consecuencia directa: `SENSITIVE_KEYS` **no interviene**, porque no hay ningún token que ocultar en
`settings` —solo hashes, y viven en su propia tabla—.

## La exención de sesión es por (método, ruta), no por ruta

Eximir «todo `/api/notifications`» dejaría **sin sesión** la lectura de no leídas y el marcado, que
son de uso interno del usuario humano. La exención debe distinguir `POST /api/notifications` de
`GET /api/notifications/unread` y `POST /api/notifications/mark-read`. Alternativa descartada: montar
el router de ingesta **después** de la capa de sesión —funcionaría por el orden de montaje, pero es
demasiado sutil para algo de seguridad: cualquiera que reordene el montaje abre un agujero sin
enterarse—.

## Se persiste antes de despachar, y el despacho es best-effort

El `201` **no espera** al WebPush: el mensaje se persiste primero y el despacho ocurre en segundo
plano. Un fallo de push **no cambia la respuesta** que ve el emisor: el mensaje ya está a salvo en la
base de datos. Consecuencia asumida: un aviso puede persistirse sin llegar a ningún dispositivo. Es
el mismo hueco *at-least-once* que el proyecto ya declara en el worker de recordatorios, y aquí es la
opción correcta: devolver un error a Uptime Kuma porque un móvil no tenía cobertura perdería el
mensaje de verdad.

## `origin` y `author` se rellenan desde el nombre de la clave cuando faltan

Como la clave es por aplicación, **el nombre ya identifica al remitente**. Si el cuerpo no trae
`origin` ni `author`, se rellenan con el nombre de la clave de ingesta que autenticó la petición. Esto
es lo que permite que la plantilla del webhook de Uptime Kuma se reduzca a **mensaje y nivel**: el
emisor no tiene que repetir quién es en cada aviso.

## Topes explícitos en todo lo que entra

El emisor es **externo**, así que lo que no se acota es un vector de agotamiento de disco. Los topes
son: cuerpo HTTP **64 KiB**, `message` ≤ **4096 bytes**, `origin` ≤ **64**, `author` ≤ **128** y
`metadata` un objeto JSON ≤ **8192 bytes** serializado. Todo lo que exceda el tope se rechaza con
`400`.

## `level` con lista blanca, `info` por defecto

`level` acepta solo `info`, `warning` y `critical`, con `info` como valor por defecto. Un nivel libre
acabaría produciendo cadenas que el frontend no sabe pintar. Cualquier otro valor devuelve `400`.

## La herramienta del agente va a la skill `pendientes`

El catálogo de skills es **cerrado (diez)** y un test exige que toda herramienta registrada pertenezca
al core o a al menos una skill. Añadir una herramienta **obliga** por tanto a tocar el catálogo y el
recuento del registry (**28 → 29**). `pendientes` es el sitio natural —«lo que tengo pendiente»— y no
se añade ni se parte ninguna skill: esto es una **consecuencia de los invariantes**, no una reapertura
de la granularidad de las skills, que sigue como se decidió.

## Una sola herramienta con operaciones, no dos

`notifications` es **una** herramienta con `list_unread` y `mark_read`, como `reminders` o `tasks`, no
dos herramientas sueltas. El permiso es `NoConfirm` para ambas: leer no leídas y marcarlas leídas es
reversible y de bajo impacto.

## Los endpoints de lectura devuelven solo no leídas

`GET /api/notifications/unread` devuelve **solo** las no leídas. El histórico de lo ya leído es otra
cosa y no se cubre aquí. Igualmente, `mark-read` es idempotente: marcar dos veces no es un error.

## El push abre la raíz de la app, no una página de bandeja

Como la bandeja es un **no-goal**, apuntar el clic del push a una ruta inexistente sería peor que
apuntar a la raíz. El clic abre `/`.

## Forma del push

El título es `[<origin>] <author>`, **sin pasar `origin` a mayúsculas** —hacerlo destrozaría nombres
como «uptime-kuma»—. El cuerpo es el mensaje. El `tag` es `notification-<id>`, de modo que un reenvío
del mismo aviso no duplica la notificación en pantalla. El icono es **`/icon-192.png`**, que es el que
sirve el proyecto (la spec original hablaba de `/assets/icon-192.png`, que no es lo que el proyecto
sirve). La `url` es `/`.

## Riesgos residuales

1. **Sin rate limiting.** Un token filtrado puede inundar la tabla `notifications` y los dispositivos.
   Los topes acotan el daño **por mensaje**, no el **número** de mensajes. Queda declarado como riesgo,
   no resuelto.
2. **La tabla `notifications` crece sin poda.** La retención y el borrado de lo viejo son otro change.
3. **El despacho best-effort puede perder avisos.** Si el WebPush falla tras devolver `201`, el
   mensaje queda persistido pero puede no llegar a ningún dispositivo.
4. **El contenido de los registros no se asevera en tests.** El repo no tiene arnés para capturar
   `tracing`, y montarlo sería otro change. Lo verificable —persistencia, validación, topes, operaciones
   del transporte, la herramienta— sí se prueba; la línea de log se documenta pero no se asevera.

## Nota

`POST /api/notifications` **no acepta sesión**: solo token de ingesta. Es deliberado. El frontend, por
tanto, no puede usar ese endpoint, y no debe intentarlo.
