## Purpose

Las cuatro herramientas de lectura de feeds de Valet —`rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`—: listan las entradas no leídas sin su contenido, traen el de una entrada concreta en texto plano sin marcarla como leída, buscan por palabra clave —también entre lo ya leído— y marcan entradas como leídas, contra la API v1 de Miniflux.

## ADDED Requirements

### Requirement: `rss_list_unread` SHALL listar las entradas no leídas sin su contenido

La herramienta `rss_list_unread` SHALL consultar `GET /v1/entries?status=unread` de Miniflux y SHALL devolver el `total` de no leídas junto a las entradas de la página, cada una con su `id`, su `title`, su `feed`, su `url`, su `published_at` y su `reading_time`, y SHALL NO incluir el `content`. Sus parámetros SHALL ser ambos opcionales: `limit` (entero, acotado al rango `1..=200`, con valor `50` cuando se omita) y `category` (nombre de categoría). Un `limit` fuera de rango SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. Cuando se informe `category`, la herramienta SHALL resolver el nombre contra `GET /v1/categories` sin distinguir mayúsculas de minúsculas y, si no coincide con ninguna, SHALL devolver un error accionable que enumere las categorías disponibles. La herramienta SHALL declarar `Permission::NoConfirm` y SHALL NO alterar el estado de nada.

#### Scenario: Un listado devuelve el total y los metadatos sin contenido
- **Given** un feed con entradas no leídas
- **When** se llama a `rss_list_unread`
- **Then** se devuelve el `total` de no leídas
- **And** cada entrada trae su `id`, `title`, `feed`, `url`, `published_at` y `reading_time`
- **And** ninguna entrada incluye el `content`
- **And** el estado de las entradas no cambia

#### Scenario: Un limit fuera de rango es un argumento inválido
- **Given** una llamada con `limit` igual a `0` o a `500`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición a Miniflux

#### Scenario: Una categoría inexistente enumera las disponibles
- **Given** una llamada con `category` igual a `Deportes`, que no existe
- **When** se ejecuta
- **Then** devuelve un error accionable
- **And** el error enumera las categorías disponibles

### Requirement: `rss_get_entry` SHALL devolver el contenido de una entrada sin marcarla como leída

La herramienta `rss_get_entry` SHALL consultar `GET /v1/entries/{id}` y SHALL devolver `id`, `title`, `feed`, `url`, `author`, `published_at`, `status` y `reading_time` junto al `content` convertido a texto plano, con las etiquetas HTML eliminadas y las entidades descodificadas, recortado a `20000` caracteres e indicando si hubo recorte. El parámetro `entry_id` SHALL ser obligatorio y SHALL ser un entero positivo; un `entry_id` ausente o no positivo SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red, y un identificador inexistente (que Miniflux responde con `404`) SHALL dar un error accionable. La herramienta SHALL declarar `Permission::NoConfirm`. El contenido devuelto SHALL tratarse como contenido de terceros no confiable.

#### Scenario: El contenido llega en texto plano y recortado si excede el máximo
- **Given** una entrada cuyo `content` es HTML con entidades y más de `20000` caracteres
- **When** se llama a `rss_get_entry` con su `entry_id`
- **Then** el `content` devuelto no contiene etiquetas HTML
- **And** las entidades aparecen descodificadas
- **And** el `content` está recortado a `20000` caracteres
- **And** se indica que hubo recorte

#### Scenario: Consultar una entrada no la marca como leída
- **Given** una entrada no leída
- **When** se llama a `rss_get_entry` con su `entry_id`
- **Then** se devuelve su contenido
- **And** la entrada sigue con estado no leída

#### Scenario: Un id inexistente es un error accionable
- **Given** un `entry_id` que no corresponde a ninguna entrada
- **When** se ejecuta
- **Then** Miniflux responde `404`
- **And** devuelve un error que indica que la entrada no existe

#### Scenario: Un entry_id ausente es un argumento inválido
- **Given** una llamada sin `entry_id` o con un `entry_id` no positivo
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición a Miniflux

### Requirement: `rss_search_entries` SHALL buscar por palabra clave sin filtrar por estado

La herramienta `rss_search_entries` SHALL consultar `GET /v1/entries` enviando los parámetros `search` y `limit` y SHALL NO enviar el parámetro `status`, de modo que la búsqueda encuentre también las entradas ya leídas. La herramienta SHALL devolver los mismos metadatos que el listado, sin el `content`. El parámetro `query` SHALL ser obligatorio y SHALL ser texto no vacío, y el parámetro `limit` SHALL ser opcional, acotado al rango `1..=200` y con valor `50` cuando se omita. Una búsqueda sin resultados SHALL devolver una lista vacía y SHALL NO ser un error. Un `query` ausente o en blanco SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. La herramienta SHALL declarar `Permission::NoConfirm`.

#### Scenario: La petición no lleva filtro de estado
- **Given** una llamada con `query` igual a `rust`
- **When** se ejecuta
- **Then** la petición lleva `search` con `rust`
- **And** la petición no lleva el parámetro `status`

#### Scenario: Una búsqueda sin resultados devuelve una lista vacía
- **Given** una consulta que no coincide con ninguna entrada
- **When** se llama a `rss_search_entries`
- **Then** devuelve una lista vacía
- **And** no devuelve ningún error

#### Scenario: Un query vacío es un argumento inválido
- **Given** una llamada sin `query` o con un `query` en blanco
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición a Miniflux

### Requirement: `rss_mark_read` SHALL marcar entradas como leídas de forma idempotente

La herramienta `rss_mark_read` SHALL enviar `PUT /v1/entries` con el cuerpo `{"entry_ids":[…],"status":"read"}` en una sola petición y SHALL devolver cuántos identificadores se pidieron. El parámetro `entry_ids` SHALL ser obligatorio y SHALL ser un array no vacío de enteros; un `entry_ids` ausente o vacío SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. La herramienta SHALL declarar `Permission::NoConfirm`. Marcar una entrada ya leída SHALL NO ser un error. Solo se admite el estado `read`.

#### Scenario: Marca varios en una petición y el cuerpo es el indicado
- **Given** una llamada con `entry_ids` igual a `[1, 2, 3]`
- **When** se ejecuta
- **Then** se envía una única petición `PUT /v1/entries`
- **And** el cuerpo es `{"entry_ids":[1,2,3],"status":"read"}`
- **And** el resultado indica que se pidieron tres identificadores

#### Scenario: Marcar dos veces no es un error
- **Given** una entrada ya marcada como leída
- **When** se vuelve a llamar a `rss_mark_read` con su `entry_ids`
- **Then** el resultado es correcto
- **And** no devuelve ningún error

#### Scenario: Una lista vacía es un argumento inválido
- **Given** una llamada sin `entry_ids` o con una lista vacía
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición a Miniflux

### Requirement: La configuración de Miniflux SHALL resolverse de settings, del entorno y de un valor por defecto

La URL base y el token de Miniflux SHALL resolverse con esta precedencia: el ajuste de `settings` (`miniflux_base_url` / `miniflux_api_key`) si tiene contenido, la variable de entorno (`MINIFLUX_BASE_URL` / `MINIFLUX_API_KEY`) en su defecto y, solo para la URL, el valor por defecto `https://miniflux.territoriolinux.es`. Las claves SHALL leerse en cada llamada, de modo que editarlas surta efecto sin reiniciar. Sin token SHALL NO intentarse ninguna petición de red: las cuatro herramientas SHALL devolver un error accionable que indique dónde configurarlo.

#### Scenario: El ajuste manda sobre el entorno
- **Given** `miniflux_api_key` en `settings` y `MINIFLUX_API_KEY` en el entorno
- **When** se ejecuta una de las herramientas
- **Then** el token usado es el de `settings`

#### Scenario: Sin ajuste se usa el entorno
- **Given** `miniflux_base_url` vacío en `settings` y `MINIFLUX_BASE_URL` en el entorno
- **When** se ejecuta una de las herramientas
- **Then** se usa la URL del entorno

#### Scenario: Sin nada configurado la URL cae al valor por defecto
- **Given** ni el ajuste ni la variable de entorno de la URL
- **When** se ejecuta una de las herramientas
- **Then** se usa `https://miniflux.territoriolinux.es`

#### Scenario: Sin token no se sale a la red
- **Given** ni el ajuste ni la variable de entorno del token
- **When** se llama a cualquiera de las cuatro herramientas
- **Then** devuelve un error accionable que indica dónde configurarlo
- **And** no se envía ninguna petición a Miniflux

### Requirement: Las cuatro herramientas del feed SHALL describirse en español y traducir los errores de Miniflux

Las descripciones de `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read` SHALL estar redactadas en español, SHALL explicar qué devuelve cada una y SHALL enumerar sus argumentos obligatorios. Las cuatro SHALL exponer un esquema de parámetros de tipo objeto. El token SHALL viajar en la cabecera `X-Auth-Token: <token>` y SHALL NO aparecer nunca en la descripción, ni en un mensaje de error, ni en un log. Los errores de Miniflux SHALL traducirse a mensajes accionables: un `401` a que el token no es válido o ha caducado, un `404` de entrada a que la entrada no existe, un fallo de red o un `5xx` a que Miniflux no está disponible, y una respuesta que no sea JSON interpretable a un error de ejecución.

#### Scenario: Las descripciones están en español y citan los obligatorios
- **Given** las definiciones de las cuatro herramientas
- **When** se inspeccionan sus descripciones
- **Then** están en español
- **And** cada una menciona sus argumentos obligatorios

#### Scenario: El token no se expone
- **Given** una llamada con el token configurado
- **When** se inspeccionan la definición y el resultado de la herramienta
- **Then** el token no aparece en ninguno de los dos

#### Scenario: Un 401 se traduce a token inválido
- **Given** un Miniflux que responde `401 unauthorized`
- **When** se ejecuta una de las herramientas
- **Then** devuelve un error que indica que el token no es válido o ha caducado

#### Scenario: Un 5xx se traduce a Miniflux no disponible
- **Given** un Miniflux que responde `503`
- **When** se ejecuta `rss_list_unread`
- **Then** el error indica que Miniflux no está disponible

## No-objetivos

Estas herramientas no marcan entradas como no leídas ni las destacan, no crean, editan ni borran feeds ni categorías, y no ofrecen ninguna interfaz de lector de noticias: se limitan a leer el estado que ya hay en Miniflux. Tampoco se apoya en `GET /v1/entries/{id}/fetch-content`, porque esa llamada reescribiría el contenido guardado en Miniflux. El único cambio de estado que se admite es marcar entradas como leídas.
