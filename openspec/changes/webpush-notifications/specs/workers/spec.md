## ADDED Requirements

### Requirement: WorkerPool SHALL incluir el ReminderPushWorker

El `WorkerPool` SHALL arrancar un `ReminderPushWorker` junto al resto de workers y SHALL detenerlo
con la señal de `shutdown`, siguiendo el mismo patrón que `EpisodicMemoryWorker`.

#### Scenario: El arranque del pool arranca el worker

- **Given** un `WorkerPool::start()`
- **When** se inicia el pool
- **Then** arranca el `ReminderPushWorker`
- **And** la referencia al worker queda retenida en el pool

#### Scenario: El shutdown detiene el worker

- **Given** un `WorkerPool` en marcha
- **When** se emite la señal de `shutdown`
- **Then** el `ReminderPushWorker` se detiene

### Requirement: El worker SHALL entregar cada recordatorio vencido como máximo una vez

En cada ciclo —de 60 segundos— el worker SHALL seleccionar los `reminders` con `status='pending'`,
`notified_at IS NULL` y `datetime <= ahora`, y al entregar uno con éxito SHALL fijar su `notified_at`.

#### Scenario: Un recordatorio vencido se envía y se marca

- **Given** un recordatorio vencido con `status='pending'` y `notified_at` a `NULL`, y al menos una suscripción
- **When** el worker ejecuta un ciclo
- **Then** se envía el aviso
- **And** el recordatorio queda con `notified_at` fijado

#### Scenario: El mismo recordatorio no se reenvía en el ciclo siguiente

- **Given** un recordatorio ya entregado y marcado con `notified_at`
- **When** el worker ejecuta otro ciclo
- **Then** el recordatorio no se selecciona
- **And** no se envía ningún aviso de nuevo

#### Scenario: Un recordatorio futuro no se toca

- **Given** un recordatorio cuyo `datetime` es posterior a `ahora`
- **When** el worker ejecuta un ciclo
- **Then** el recordatorio no se selecciona
- **And** su `notified_at` sigue a `NULL`

### Requirement: El worker SHALL NO consumir un recordatorio cuando no hay ninguna suscripción

Sin dispositivos suscritos el worker SHALL NOT marcar `notified_at`: el recordatorio SHALL entregarse
cuando aparezca una suscripción, si sigue dentro de la ventana.

#### Scenario: Sin suscripciones no hay envío ni marca

- **Given** un recordatorio vencido y ninguna suscripción registrada
- **When** el worker ejecuta un ciclo
- **Then** no se envía ningún aviso
- **And** el recordatorio sigue con `notified_at` a `NULL`

#### Scenario: Al aparecer una suscripción se entrega el recordatorio vigente

- **Given** un recordatorio vencido que sigue dentro de la ventana y ninguna suscripción
- **When** se registra un dispositivo y el worker ejecuta el ciclo siguiente
- **Then** el recordatorio se entrega
- **And** su `notified_at` queda fijado

### Requirement: Los recordatorios vencidos fuera de la ventana SHALL descartarse sin entregarse

Si `ahora - datetime` supera `push_reminder_grace_hours` —por defecto 24 horas—, el worker SHALL
fijar `notified_at` y registrar un aviso, sin enviar, para no inundar de avisos antiguos al primer
dispositivo que se suscriba.

#### Scenario: Un recordatorio vencido dentro de la ventana se entrega

- **Given** un recordatorio vencido hace menos de la ventana configurada
- **When** el worker ejecuta un ciclo
- **Then** el recordatorio se entrega
- **And** su `notified_at` queda fijado

#### Scenario: Un recordatorio vencido fuera de la ventana se descarta con un aviso

- **Given** un recordatorio vencido hace más que la ventana configurada
- **When** el worker ejecuta un ciclo
- **Then** no se envía ningún aviso
- **And** su `notified_at` queda fijado
- **And** se registra un aviso de descarte

#### Scenario: El umbral se lee del ajuste y con '0' todo queda fuera de ventana

- **Given** `push_reminder_grace_hours` con el valor `'0'`
- **When** el worker evaluaba un recordatorio ya vencido
- **Then** se considera fuera de la ventana
- **And** se marca sin enviarse

### Requirement: El payload del aviso SHALL identificar el recordatorio

El payload del aviso SHALL llevar `title` «Recordatorio», `body` el texto del recordatorio, `url` `/`
y `tag` `reminder-<id>`, de modo que el `tag` evite que dos avisos del mismo recordatorio se apilen.

#### Scenario: El payload contiene el texto del recordatorio

- **Given** un recordatorio con su texto
- **When** el worker construye el payload del aviso
- **Then** el `body` es el texto del recordatorio
- **And** el `title` es «Recordatorio»
- **And** el `url` es `/`

#### Scenario: El tag incluye el id del recordatorio

- **Given** un recordatorio con su `id`
- **When** el worker construye el payload del aviso
- **Then** el `tag` es `reminder-<id>`

### Requirement: El envío SHALL ser inyectable para poder probarlo sin red

El worker SHALL depender de una abstracción `PushSender` y SHALL NO construir el cliente HTTP por
dentro, de modo que las pruebas puedan sustituirlo.

#### Scenario: Un PushSender de prueba registra las llamadas

- **Given** un `PushSender` de prueba inyectado en el worker
- **When** el worker entrega un recordatorio
- **Then** el sender de prueba registra la llamada de envío

#### Scenario: El worker no falla si el sender devuelve error

- **Given** un `PushSender` de prueba que devuelve un error
- **When** el worker intenta entregar un recordatorio
- **Then** el worker no se cae
- **And** registra el error
