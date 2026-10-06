# message Specification

## Purpose
Modelo de mensaje enriquecido de Valet: estimación heurística de tokens Markdown, umbral de colapso configurable, nuevas columnas de migración y campos opcionales de ubicación y herramientas usadas.

## Requirements

### Requirement: estimate_markdown_tokens_heuristic SHALL compute token estimate via word + markdown heuristic
**Given** a text string  
**When** `estimate_markdown_tokens_heuristic(text)` is called  
**Then** it SHALL return `(word_count * 1.33).trunc() + md_symbol_count`  
**Where** word boundaries are whitespace or punctuation/Markdown symbols, and md_symbols are characters in `# * ` _ [] () > - + ! . , ; : ? " ' / \ = ~ |`

#### Scenario: empty text
**Given** empty text ""  
**When** `estimate_markdown_tokens_heuristic("")` is called  
**Then** it SHALL return 0

#### Scenario: plain text
**Given** text "Hola" (1 word)  
**When** `estimate_markdown_tokens_heuristic("Hola")` is called  
**Then** it SHALL return 1

#### Scenario: markdown heading
**Given** text "# Hello World" (2 words, `#` symbol)  
**When** `estimate_markdown_tokens_heuristic("# Hello World")` is called  
**Then** it SHALL return 3

#### Scenario: markdown bold
**Given** text "Some **bold** text" (3 words, 4 `*` symbols)  
**When** `estimate_markdown_tokens_heuristic("Some **bold** text")` is called  
**Then** it SHALL return 7

#### Scenario: markdown list
**Given** text "- Item one\n- Item two" (4 words, 2 `-` symbols)  
**When** `estimate_markdown_tokens_heuristic(text)` is called  
**Then** it SHALL return 7

#### Scenario: inline code
**Given** text "Use `let x = 1;`" (4 words, 4 md symbols)  
**When** `estimate_markdown_tokens_heuristic(text)` is called  
**Then** it SHALL return 9

### Requirement: Message model SHALL include new fields
**Given** a message stored in the database  
**Then** it SHALL have fields `tokens_count`, `collapsed_content`, `collapsed_tokens_count`, `is_indexed`, `summary_ref`

#### Scenario: Creation computes tokens_count automatically
**Given** content "Hola, ¿cómo estás?" (20 chars)  
**When** saved via `MessagesRepo::create`  
**Then** `msg.tokens_count` SHALL equal `estimate_markdown_tokens_heuristic(content)`  
**And** `msg.collapsed_content` SHALL be `None`  
**And** `msg.collapsed_tokens_count` SHALL be `0`  
**And** `msg.is_indexed` SHALL be `false`

### Requirement: Collapse threshold SHALL be configurable
**Given** `COLLAPSE_THRESHOLD_TOKENS` env var  
**When** `Config::from_env()` is called  
**Then** `collapse_threshold_tokens` SHALL take the env var value or default to 2000

#### Scenario: Short message SHALL NOT trigger collapse
**Given** `collapse_threshold_tokens` = 2000  
**Given** a message with 100 chars  
**When** saved  
**Then** `tokens_count` SHALL be < 2000  
**And** the collapse callback SHALL NOT be invoked

#### Scenario: Long message SHALL trigger collapse
**Given** `collapse_threshold_tokens` = 2000  
**Given** a message with sufficient content to exceed the threshold  
**When** saved  
**Then** `tokens_count` SHALL be >= 2000  
**And** the collapse callback SHALL be invoked with the message `id`

### Requirement: Migration SHALL add new columns idempotently
**Given** a database with an existing `messages` table  
**When** `run_migrations()` is executed  
**Then** the table SHALL have columns `tokens_count`, `collapsed_content`, `collapsed_tokens_count`, `is_indexed`, `summary_ref`  
**And** running migrations twice SHALL NOT fail

#### Scenario: New columns exist after migration
**Given** a fresh in-memory database  
**When** `run_migrations()` is executed  
**Then** `PRAGMA table_info(messages)` SHALL include `tokens_count`, `collapsed_content`, `collapsed_tokens_count`, `is_indexed`, `summary_ref`

#### Scenario: Migration is idempotent
**Given** a database where `run_migrations()` has already been executed
**When** `run_migrations()` is executed again
**Then** it SHALL succeed without error

### Requirement: Message list endpoint SHALL use configurable page size

**Given** una petición GET `/api/conversations/{id}/messages`
**When** no se especifica `limit` en query params
**Then** el handler SHALL leer `message_page_size` de settings y usarlo como límite por defecto
**And** si el setting no existe, SHALL usar 50 como fallback

#### Scenario: GET sin limit usa el setting
**Given** `message_page_size` = 25 en settings
**When** GET `/api/conversations/conv-id/messages`
**Then** devuelve 25 mensajes

#### Scenario: GET con limit explícito sobreescribe el setting
**Given** `message_page_size` = 25 en settings
**When** GET `/api/conversations/conv-id/messages?limit=10`
**Then** devuelve 10 mensajes

### Requirement: Message model SHALL include optional location field

**Given** un mensaje en la base de datos  
**When** se recupera via API  
**Then** el campo `location` SHALL ser `Option<String>`  
**And** SHALL ser `null` cuando no hay ubicación configurada  
**And** SHALL contener la dirección textual (ej. "Silla, Valencia, España") cuando está disponible

#### Scenario: Location se persiste al crear mensaje con ubicación disponible
**Given** settings contienen `latitude`, `longitude` y `location_name`  
**When** `MessagesRepo::create()` es llamado  
**Then** el mensaje creado SHALL incluir `location` con el valor de `location_name` de settings

#### Scenario: Location es null cuando no hay ubicación
**Given** settings NO contienen `latitude` ni `location_name`  
**When** `MessagesRepo::create()` es llamado  
**Then** el mensaje creado SHALL tener `location = None`

#### Scenario: Migration añade columna location idempotentemente
**Given** una base de datos con tabla `messages` existente  
**When** `run_migrations()` se ejecuta  
**Then** la tabla SHALL tener columna `location TEXT` nullable  
**And** ejecutar migrations dos veces SHALL NO fallar

#### Scenario: LIST devuelve location
**Given** un mensaje con `location = "Madrid, España"`  
**When** `list_all()` es llamado  
**Then** el mensaje devuelto SHALL incluir `location = Some("Madrid, España")`

#### Scenario: find_by_id devuelve location
**Given** un mensaje con `location = "Barcelona"`  
**When** `find_by_id(msg_id)` es llamado  
**Then** el mensaje devuelto SHALL incluir `location = Some("Barcelona")`

### Requirement: Message model SHALL include optional tools_used field

**Given** un mensaje almacenado en la base de datos  
**When** se recupera via API  
**Then** el campo `tools_used` SHALL ser `Option<String>`  
**And** SHALL ser `null` cuando no hay herramientas utilizadas  
**And** SHALL contener el string formateado con las herramientas usadas (ej. `"(2) calendar::get_events, weather::get_weather"`) cuando hay herramientas

#### Scenario: tools_used se persiste al crear mensaje assistant con herramientas
**Given** un mensaje assistant con herramientas usadas  
**When** `MessagesRepo::create()` es llamado con `tools_used = Some("(2) calendar::get_events, weather::get_weather")`  
**Then** el mensaje creado SHALL incluir `tools_used = Some("(2) calendar::get_events, weather::get_weather")`

#### Scenario: tools_used es null cuando no hay herramientas
**Given** un mensaje assistant sin herramientas  
**When** `MessagesRepo::create()` es llamado con `tools_used = None`  
**Then** el mensaje creado SHALL tener `tools_used = None`

#### Scenario: Migration añade columna tools_used idempotentemente
**Given** una base de datos con tabla `messages` existente  
**When** `run_migrations()` se ejecuta  
**Then** la tabla SHALL tener columna `tools_used TEXT` nullable  
**And** ejecutar migrations dos veces SHALL NO fallar

#### Scenario: LIST devuelve tools_used
**Given** un mensaje con `tools_used = "(2) calendar::get_events"`  
**When** `list_all()` es llamado  
**Then** el mensaje devuelto SHALL incluir `tools_used = Some("(2) calendar::get_events")`

#### Scenario: find_by_id devuelve tools_used
**Given** un mensaje con `tools_used = "weather::get_weather"`  
**When** `find_by_id(msg_id)` es llamado  
**Then** el mensaje devuelto SHALL incluir `tools_used = Some("weather::get_weather")`

### Requirement: Message model SHALL include optional widgets field

**Given** un mensaje almacenado en la base de datos
**When** se recupera via API
**Then** el campo `widgets` SHALL ser `Option<Value>` (JSON)
**And** SHALL ser `null` cuando el mensaje no mostró widgets
**And** SHALL contener la lista `[{ id, name, data }]` de los widgets renderizados por el asistente cuando los haya

#### Scenario: set_widgets persiste la lista de widgets
**Given** un mensaje assistant recién creado
**When** se llama `MessagesRepo::set_widgets(id, [{ id: "w1", name: "LocationWidget", data: { latitude: 1.0, longitude: 2.0 } }])`
**Then** `find_by_id(id)` devuelve `widgets` con esa lista

#### Scenario: widgets es null cuando no hay
**Given** un mensaje sin widgets
**When** se recupera
**Then** `widgets` SHALL ser `null`

#### Scenario: Migration añade columna widgets idempotentemente
**Given** una base de datos con tabla `messages` existente
**When** `run_migrations()` se ejecuta
**Then** la tabla SHALL tener columna `widgets TEXT` nullable
**And** ejecutar migrations dos veces SHALL NO fallar

#### Scenario: LIST devuelve widgets
**Given** un mensaje con `widgets = [{ id: "w1", name: "Checklist", data: {} }]`
**When** `list_all()` es llamado
**Then** el mensaje devuelto SHALL incluir esa lista en `widgets`
