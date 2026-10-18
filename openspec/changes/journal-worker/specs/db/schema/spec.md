## MODIFIED Requirements

### Requirement: La migración SHALL ampliar los valores admitidos de `llm_requests.kind` sin perder filas

Una migración nueva SHALL ampliar el `CHECK` de `llm_requests.kind` para admitir `'journal'`, además
de `'timeline'`, `'chat'`, `'router'`, `'archivist'`, `'consolidator'` y `'collapse'`. Como SQLite no
permite alterar un `CHECK` en línea, la migración SHALL reconstruir la tabla y SHALL preservar
**todas** las filas existentes, sea cual sea su `kind`.

#### Scenario: La tabla acepta el nuevo origen
- **Given** la migración aplicada
- **When** se inserta una fila de `llm_requests` con `kind = 'timeline'`
- **Then** la inserción se acepta

#### Scenario: Las filas previas sobreviven
- **Given** una base de datos con filas de los cinco valores anteriores de `kind`
- **When** se ejecuta la migración
- **Then** todas las filas siguen ahí
- **And** sus valores de `kind` no cambian

#### Scenario: Un valor fuera del conjunto sigue rechazándose
- **Given** la migración aplicada
- **When** se intenta insertar una fila con un `kind` desconocido
- **Then** la base de datos lo rechaza

#### Scenario: La migración es idempotente
- **Given** la migración ya aplicada
- **When** se ejecuta de nuevo
- **Then** no hay error
- **And** no se duplica ninguna fila

#### Scenario: La tabla acepta el origen de la bitácora
- **Given** la migración aplicada
- **When** se inserta una fila de `llm_requests` con `kind = 'journal'`
- **Then** la inserción se acepta

#### Scenario: Las filas del timeline sobreviven a la segunda reconstrucción
- **Given** una base de datos con filas de `llm_requests` de los seis valores anteriores de `kind`,
  incluidas filas con `kind = 'timeline'`
- **When** se ejecuta la migración que amplía el `CHECK` para `'journal'`
- **Then** todas las filas siguen ahí
- **And** ninguna fila con `kind = 'timeline'` se pierde ni cambia de valor

## ADDED Requirements

### Requirement: La migración SHALL sembrar los ajustes de la bitácora sin pisar una personalización

Una migración nueva SHALL sembrar en `settings`, de forma idempotente, los ajustes de la bitácora:
`journal_prompt` con el prompt del relato, `journal_enabled` con `'true'` y `journal_hour` con `'4'`.
La siembra SHALL seguir el idioma de `20261010000004_timeline_prompts.sql`: SHALL rellenar una clave
ausente o vacía y SHALL NOT sobrescribir **nunca** un valor ya presente —una personalización del
usuario, aunque el valor sea el sembrado por una migración anterior—.

La marca de progreso `journal_last_judged_day` SHALL NOT sembrarse: su ausencia es el estado «nunca se
ha juzgado nada» y el worker arranca por el último día cerrado (ver `workers`). Sembrarla con el valor
equivocado haría que el diario empezara por donde no toca o que no empezara nunca.

#### Scenario: Tras migrar existen las tres claves con sus valores

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** `settings` contiene `journal_prompt` con un valor no vacío
- **And** contiene `journal_enabled` con `'true'`
- **And** contiene `journal_hour` con `'4'`

#### Scenario: Una reejecución no sobrescribe un valor personalizado

- **Given** un `journal_prompt` editado por el usuario, un `journal_enabled` a `'false'` y un
  `journal_hour` a `'23'`
- **When** se vuelve a ejecutar la migración
- **Then** los tres valores se conservan tal cual
- **And** no se produce ningún error

#### Scenario: Una clave ausente se rellena

- **Given** un `settings` sin la clave `journal_prompt`
- **When** se ejecuta la migración
- **Then** `journal_prompt` existe con el prompt del relato

#### Scenario: La marca de progreso no se siembra

- **Given** una base de datos recién migrada
- **When** se consulta `settings`
- **Then** NO existe la clave `journal_last_judged_day`
- **And** el worker arranca por el último día cerrado

#### Scenario: El prompt sembrado queda listo para editarse

- **Given** una base de datos recién migrada
- **When** se consulta `journal_prompt` por `GET /api/settings`
- **Then** la clave aparece con su valor
- **And** se puede cambiar por `PUT /api/settings` como cualquier otra
