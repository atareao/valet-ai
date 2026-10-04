# Spec Delta: tools/reminders

## ADDED Requirements

### Requirement: RemindersTool SHALL manage reminders through four operations

La tool `reminders` SHALL despachar por el argumento `operation` y soportar `set_reminder`,
`list_reminders`, `dismiss_reminder` y `snooze_reminder` sobre la tabla `reminders` a través de
`RemindersRepo`. `set_reminder` y `list_reminders` SHALL resolver `profile_id` desde el argumento, usando
`default` si falta; `profile_id` NO SHALL exponerse en `parameters()`. `set_reminder` SHALL crear el
recordatorio con estado `pending`; `dismiss_reminder` y `snooze_reminder` operan por `id`.

**Given** una `RemindersTool` con acceso a la BD
**When** se ejecuta `reminders` con una `operation`
**Then** DEBE operar sobre `RemindersRepo` y devolver datos serializados

#### Scenario: set_reminder crea un recordatorio pendiente
**Given** una `RemindersTool`
**When** se ejecuta con `{"operation":"set_reminder","text":"Llamar al dentista","datetime":"2026-10-05T09:00:00Z"}`
**Then** DEBE crear la fila vía `RemindersRepo::create` con estado `pending`
**And** DEBE devolver el recordatorio creado

#### Scenario: profile_id por defecto y no expuesto
**Given** una `RemindersTool`
**When** se ejecuta `set_reminder` sin `profile_id`
**Then** DEBE usar `default`
**And** `parameters()` NO DEBE incluir `profile_id`

#### Scenario: list_reminders filtra por estado
**Given** una `RemindersTool` con recordatorios `pending` y `dismissed`
**When** se ejecuta con `{"operation":"list_reminders","status":"pending"}`
**Then** DEBE devolver solo los recordatorios con ese estado

#### Scenario: dismiss_reminder descarta un recordatorio
**Given** un recordatorio existente
**When** se ejecuta con `{"operation":"dismiss_reminder","id":"<id>"}`
**Then** DEBE marcarlo como descartado vía `RemindersRepo::dismiss`
**And** DEBE devolver `{id}`

#### Scenario: snooze_reminder pospone con 10 minutos por defecto
**Given** un recordatorio existente
**When** se ejecuta con `{"operation":"snooze_reminder","id":"<id>"}` sin `minutes`
**Then** DEBE posponerlo 10 minutos vía `RemindersRepo::snooze`
**And** DEBE devolver `{id, snoozed_minutes, new_datetime}`

### Requirement: RemindersTool SHALL always request Notify permission

La tool `reminders` SHALL declarar permiso `Notify` para todas sus operaciones, sin aprobación
explícita ni ejecución silenciosa.

**Given** una `RemindersTool`
**When** se consulta `permission()`
**Then** DEBE devolver `Notify` con independencia de la `operation`

#### Scenario: Permiso Notify en todas las operaciones
**Given** una `RemindersTool`
**When** la `operation` es `set_reminder`, `list_reminders`, `dismiss_reminder` o `snooze_reminder`
**Then** `permission()` DEBE devolver `Notify`
