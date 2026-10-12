## ADDED Requirements

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
