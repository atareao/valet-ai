## Purpose

La interfaz de las notificaciones WebPush en Valet: el service worker que recibe los avisos y
gestiona el clic, su registro tolerante al arrancar la aplicación, la activación y desactivación
desde Ajustes, y la conversión base64url necesaria para suscribir el navegador con la clave pública
VAPID.

## ADDED Requirements

### Requirement: El service worker SHALL atender `push` y `notificationclick` y SHALL NO interceptar `fetch`

El service worker SHALL vivir en `frontend/public/sw.js` —Vite lo copia tal cual a `dist/` y el
backend lo sirve como estático en `/sw.js`— y SHALL registrar los handlers `push` y
`notificationclick`. SHALL NO registrar un handler de `fetch`, de modo que no pueda servir ficheros
en caché ni romper el HMR de Vite; por eso SHALL registrarse también en desarrollo.

#### Scenario: Un push sin datos no muestra notificación

- **Given** un evento `push` sin `event.data`
- **When** el service worker lo recibe
- **Then** no muestra ninguna notificación

#### Scenario: Un push con payload muestra la notificación

- **Given** un evento `push` cuyo payload es `{"title": "...", "body": "...", "url": "...", "tag": "..."}`
- **When** el service worker lo recibe
- **Then** llama a `showNotification` con el título y el cuerpo del payload
- **And** usa `data.url` como destino
- **And** usa el `tag` del payload

#### Scenario: Un clic en la notificación abre o enfoca la pestaña

- **Given** una notificación mostrada
- **When** se produce `notificationclick`
- **Then** cierra la notificación
- **And** enfoca una pestaña ya abierta en esa URL o abre una nueva

### Requirement: `main.tsx` SHALL registrar `/sw.js` de forma tolerante

`main.tsx` SHALL registrar el service worker en `/sw.js`. Si `navigator.serviceWorker` no existe,
SHALL NO hacer nada. Si `register()` falla, SHALL registrar el fallo en consola y SHALL NO romper la
aplicación.

#### Scenario: Un navegador con soporte registra el service worker

- **Given** un navegador con `navigator.serviceWorker`
- **When** arranca la aplicación
- **Then** se llama a `register('/sw.js')`

#### Scenario: Un navegador sin soporte no intenta registrar nada

- **Given** un navegador sin `navigator.serviceWorker`
- **When** arranca la aplicación
- **Then** no se llama a `register`
- **And** no se produce ningún error

#### Scenario: Un registro fallido no rompe la aplicación

- **Given** un `register('/sw.js')` que se rechaza
- **When** arranca la aplicación
- **Then** la aplicación sigue montada
- **And** el fallo queda registrado en consola

### Requirement: El frontend SHALL permitir activar y desactivar las notificaciones desde Ajustes

`SettingsDialog` SHALL exponer un tab «Notificaciones» con un `Switch`. Activarlo SHALL pedir permiso
con `Notification.requestPermission()`, suscribir el navegador con la clave pública de
`GET /api/push/vapid-key` como `applicationServerKey` (desde base64url) y publicar la suscripción en
`POST /api/push/subscriptions`. Desactivarlo SHALL llamar primero a `DELETE /api/push/subscriptions`
con el `endpoint` y después cancelar la suscripción del navegador con `subscription.unsubscribe()`
—en este orden, para que un fallo parcial deje el estado consistente «el backend no tiene fila»—.
Si el `POST` de alta falla tras suscribir el navegador, SHALL deshacer la suscripción del navegador
antes de propagar el error. El estado mostrado SHALL distinguir cinco casos: sin soporte, permiso
aún no solicitado (`Notification.permission === "default"`), permiso denegado, permiso concedido
sin suscripción, y suscrito. La consulta de estado SHALL NO esperar a `navigator.serviceWorker.ready`
(usa `getRegistration()`), para no colgar cuando el registro del worker ha fallado.

#### Scenario: Activar pide permiso, suscribe y publica

- **Given** el tab «Notificaciones» con el soporte del navegador disponible y permiso no decidido
- **When** el usuario activa el `Switch`
- **Then** se pide permiso con `Notification.requestPermission()`
- **And** se concede el permiso
- **And** se crea la suscripción del navegador
- **And** se envía un `POST /api/push/subscriptions`

#### Scenario: Un permiso aún no solicitado se muestra como tal

- **Given** un navegador con `Notification.permission === "default"` y sin suscripción
- **When** se abre el tab «Notificaciones»
- **Then** el estado mostrado es «aún no has dado permiso», no «permiso concedido»
- **And** el `Switch` aparece apagado

#### Scenario: Un fallo del POST deshace la suscripción del navegador

- **Given** un permiso concedido y una suscripción del navegador recién creada
- **When** el `POST /api/push/subscriptions` se rechaza
- **Then** se cancela la suscripción del navegador con `subscription.unsubscribe()`
- **And** se propaga el error a la UI

#### Scenario: Un permiso denegado muestra un mensaje y no publica nada

- **Given** un usuario que deniega el permiso de notificaciones
- **When** activa el `Switch`
- **Then** se muestra un mensaje claro de permiso denegado
- **And** no se envía ningún `POST /api/push/subscriptions`

#### Scenario: Desactivar cancela la suscripción y borra en el servidor

- **Given** un navegador suscrito
- **When** el usuario desactiva el `Switch`
- **Then** se envía un `DELETE /api/push/subscriptions` con el `endpoint`
- **And** después se llama a `subscription.unsubscribe()`

#### Scenario: Un fallo del DELETE no desuscribe el navegador

- **Given** un navegador suscrito
- **When** el `DELETE /api/push/subscriptions` se rechaza
- **Then** NO se cancela la suscripción del navegador
- **And** se propaga el error a la UI

#### Scenario: Un navegador sin PushManager muestra el switch deshabilitado

- **Given** un navegador sin `PushManager`
- **When** se renderiza el tab «Notificaciones»
- **Then** el `Switch` aparece deshabilitado
- **And** se muestra una explicación

#### Scenario: La carga inicial refleja la suscripción existente

- **Given** un navegador que ya tenía una suscripción
- **When** se abre el tab «Notificaciones»
- **Then** el estado mostrado es «suscrito»

### Requirement: La conversión base64url→Uint8Array SHALL manejar padding y alfabeto url-safe

La conversión de la clave pública base64url al `Uint8Array` que consume `applicationServerKey` SHALL
aceptar el alfabeto url-safe (`-` y `_`) y SHALL manejar correctamente la ausencia o la presencia de
padding.

#### Scenario: Una cadena url-safe sin padding produce los bytes correctos

- **Given** una cadena base64url con `-` y `_` y sin padding
- **When** se convierte a `Uint8Array`
- **Then** los bytes resultantes son los esperados

#### Scenario: Una longitud múltiplo de 4 no añade padding

- **Given** una cadena cuya longitud ya es múltiplo de 4
- **When** se convierte a `Uint8Array`
- **Then** no se añade padding adicional
- **And** los bytes resultantes son los esperados

#### Scenario: Una cadena vacía produce un array vacío

- **Given** una cadena vacía
- **When** se convierte a `Uint8Array`
- **Then** el resultado es un array vacío
