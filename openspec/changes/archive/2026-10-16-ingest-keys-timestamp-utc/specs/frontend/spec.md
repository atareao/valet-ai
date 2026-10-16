## MODIFIED Requirements

### Requirement: SettingsDialog SHALL permitir crear y revocar claves de ingesta desde la pestaña API Keys

En la pestaña **API Keys** del `SettingsDialog` —que ya existe y hoy muestra las claves **salientes**
(OpenWeatherMap, Google Places, Brave, apimail y Miniflux)— se SHALL añadir una sección separada de
claves **entrantes**, para las aplicaciones que envían avisos. Esa sección SHALL ofrecer un formulario
para crear una clave a partir de un **nombre**, la lista de claves existentes y la revocación de cada
una. La lista SHALL mostrar de cada clave el nombre, el prefijo, la fecha de alta, el último uso y si
está revocada, y SHALL NOT mostrar nunca el token: solo el prefijo. El **token SHALL mostrarse una
única vez**, en el momento del alta, con un aviso explícito de que no se volverá a mostrar. NO SHALL
añadirse una pestaña nueva de primer nivel, porque el diálogo ya tiene siete y existe un requisito de
que quepan sin desbordamiento. Los avisos al usuario SHALL emitirse mediante la API contextual de
antd ya usada por el proyecto (`App.useApp()`, ver la capability `ui-feedback`), NO mediante la API
estática de antd.

Las fechas que la lista muestra —la de **alta** (`created_at`) y la de **último uso**
(`last_used_at`)— SHALL interpretarse como **UTC** cuando lleguen en el formato crudo de SQLite
(`"YYYY-MM-DD HH:MM:SS"`: separador de espacio, sin `T` y sin zona horaria; es el valor que emite
`datetime('now')` y que el backend reenvía tal cual). Un valor ya en ISO-8601 (con `T` y zona) SHALL
respetarse sin tocarlo. Un valor **ausente o no parseable** SHALL mostrarse como una **ausencia
legible** (`—` o equivalente) y SHALL NOT mostrarse nunca como el valor crudo ni como «Invalid
Date». La marca temporal SHALL ser **independiente de la zona horaria** de la máquina del usuario,
de modo que el mismo instante no cambie según el navegador que lo pinte (WebKit rechaza el crudo de
SQLite; V8 lo interpreta como hora local).

#### Scenario: Crear una clave muestra el token una sola vez con el aviso

- **Given** un usuario en la pestaña API Keys del `SettingsDialog`
- **When** crea una clave de ingesta con un nombre
- **Then** la interfaz muestra el token recién creado
- **And** muestra un aviso explícito de que no se volverá a mostrar

#### Scenario: La lista no contiene el token

- **Given** una clave de ingesta ya creada
- **When** se inspecciona la lista de claves
- **Then** la lista muestra el prefijo de la clave
- **And** la lista NO muestra el token

#### Scenario: Revocar marca la clave como revocada

- **Given** una clave existente
- **When** el usuario la revoca
- **Then** la lista marca la clave como revocada

#### Scenario: Un fallo del backend al crear se comunica sin dejar la clave a medias

- **Given** un usuario que crea una clave y un backend que responde con error
- **When** falla la creación
- **Then** la interfaz comunica el fallo con un aviso
- **And** NO deja la clave creada a medias en la lista

#### Scenario: El crudo de SQLite se interpreta como UTC

- **Given** una clave con `last_used_at` en el formato crudo de SQLite `"2026-10-12 13:55:33"`
- **When** se renderiza la lista de claves
- **Then** la celda de «último uso» muestra ese instante como UTC (`2026-10-12T13:55:33Z`)
- **And** la celda de «alta» (`created_at`) se interpreta con la misma regla

#### Scenario: La marca no depende de la zona horaria del navegador

- **Given** una clave con `last_used_at` en el formato crudo de SQLite
- **When** la lista se renderiza en WebKit y en V8
- **Then** ambas muestran el mismo instante, sin desfase
- **And** ninguna lo lee como hora local

#### Scenario: Un valor no parseable se muestra como ausencia

- **Given** una clave con `last_used_at` no parseable (p. ej. `"no es una fecha"`)
- **When** se renderiza la lista de claves
- **Then** la celda de «último uso» muestra `—`
- **And** nunca muestra «Invalid Date»

#### Scenario: Un valor ausente no se muestra como el crudo

- **Given** una clave con `last_used_at` nulo
- **When** se renderiza la lista de claves
- **Then** la celda de «último uso» muestra `—`
- **And** nunca muestra el valor crudo del backend
