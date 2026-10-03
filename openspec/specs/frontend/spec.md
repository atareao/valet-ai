# Frontend UI Polish — Componentes, estilos globales y tipografía

## Purpose

Interfaz de usuario de Valet: chat, agenda, tareas, estadísticas y ajustes, con estilos globales y tipografía configurable.

## Contracts

### Configuración de tema (theme.ts)

```typescript
export const valetTheme: ThemeConfig = {
  token: {
    colorPrimary: '#1677ff',
    borderRadius: 6,
    colorBgContainer: '#141414',
    colorBgLayout: '#000000',
    colorText: '#ffffff',
    colorBgElevated: '#1f1f1f',
  },
  components: {
    Layout: {
      siderBg: '#001529',
      headerBg: '#141414',
      bodyBg: '#000000',
    },
    Menu: {
      darkItemBg: '#001529',
      darkItemSelectedBg: '#1677ff',
    },
  },
}
```

Sin cambios en estructura, pero se añade un token `fontSize` variable (desde settings).

### Estilos globales (global.css)

```css
/* Reset de márgenes del body que antd deja por defecto */
body {
  margin: 0;
  padding: 0;
  background: #000;
}

/* Custom scrollbar — oscura, delgada, integrada */
::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: rgba(255,255,255,0.15);
  border-radius: 3px;
}
::-webkit-scrollbar-thumb:hover {
  background: rgba(255,255,255,0.25);
}

/* Firefox scrollbar */
* {
  scrollbar-width: thin;
  scrollbar-color: rgba(255,255,255,0.15) transparent;
}

/* Font stack mobile-first */
:root {
  --font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
    'Helvetica Neue', Arial, 'Noto Sans', sans-serif, 'Apple Color Emoji',
    'Segoe UI Emoji';
  --font-size-base: 16px;
}
```

### Tipografía variable desde settings

El usuario puede configurar `font_size` (number, 12-24, default 16) en el panel de ajustes.
El valor se almacena como setting server-side (`settings.font_size`) y se aplica via CSS variable en el root del layout.

### AppLayout — sin marco blanco

El `Layout.Content` de antd NO debe tener padding/margin blanco.
Se asegura que el fondo del content es `#000` (ya configurado) y que no hay
márgenes residuales del body o de los estilos por defecto de antd.

## Escenarios

### Escenario 1: No hay marco blanco alrededor del chat
**Given** el Layout.Content con bodyBg: '#000000'  
**When** se renderiza AppLayout  
**Then** NO hay padding ni margen blanco visible  
**And** el fondo del área de chat es negro (#000)  
**And** no hay bordes ni sombras blancas alrededor del contenedor de mensajes

### Escenario 2: Scrollbar oscura integrada
**Given** la aplicación renderizada con estilos globales  
**When** el contenido del chat excede la altura visible  
**Then** la scrollbar es de 6px de ancho  
**And** el track es transparente  
**And** el thumb es rgba(255,255,255,0.15) con border-radius 3px  
**And** en Firefox usa `scrollbar-width: thin`

### Escenario 3: Tipografía mobile-first
**Given** los estilos globales cargados  
**When** se renderiza cualquier texto en la app  
**Then** la font-family usa el stack mobile-first (-apple-system, etc.)  
**And** el font-size base es 16px  
**And** en móvil (viewport < 768px) la experiencia es legible sin zoom

### Escenario 4: Usuario puede cambiar tamaño de fuente desde Ajustes
**Given** el panel de ajustes abierto  
**When** el usuario modifica `font_size` (slider de 12 a 24)  
**And** guarda los cambios  
**Then** el tamaño de fuente del chat se actualiza al valor guardado  
**And** persiste entre recargas

### Escenario 5: Valor por defecto de font_size es 16
**Given** settings sin `font_size`  
**When** se carga la app  
**Then** el font-size aplicado es 16px

## Requirements

### Requirement: CalendarView header responsivo

El header del calendario SHALL apilar sus controles en vertical en viewports pequeños y mostrarlos en fila en desktop.

#### Scenario: Header se apila verticalmente en móvil
**Given** el viewport es < 768px  
**When** se abre el modal de agenda  
**Then** el selector de categoría y botón "New Event" están en columna (stack vertical)  
**And** ocupan el ancho completo disponible

#### Scenario: Header en fila en desktop
**Given** el viewport es ≥ 768px  
**When** se abre el modal de agenda  
**Then** el selector y botón están en fila horizontal (como ahora)

### Requirement: Modales con ancho dinámico

Los modales de evento SHALL adaptar su ancho al viewport, ocupando el ancho completo menos padding en móvil.

#### Scenario: EventModal se adapta en móvil
**Given** el viewport es < 768px  
**When** se abre EventModal  
**Then** el modal ocupa `'100vw'` menos padding (16px a cada lado)

#### Scenario: EventDetail se adapta en móvil
**Given** el viewport es < 768px  
**When** se abre EventDetail  
**Then** el modal ocupa `'100vw'` menos padding  
**And** las descripciones se muestran sin borde (mejor legibilidad)

### Requirement: Lista de eventos del día responsiva

La lista de eventos del día SHALL apilar cada evento a ancho completo con título y hora en vertical en móvil.

#### Scenario: Eventos del día se apilan en móvil
**Given** el viewport es < 768px  
**When** se selecciona una fecha con eventos  
**Then** cada evento ocupa el ancho completo  
**And** título y hora están en vertical en vez de horizontal

### Requirement: StatsDashboard SHALL display cost and tokens by model

StatsDashboard SHALL mostrar el coste y los tokens por modelo mediante un gráfico de barras y una tabla.

**Given** el endpoint `/api/stats/llm/by-model` devuelve datos
**When** se renderiza StatsDashboard
**Then** muestra un gráfico de barras (Chart.js) con coste por modelo
**And** una tabla con modelo, calls, tokens, coste

#### Scenario: Gráfico con 2 modelos
**Given** by-model devuelve `[{model: "gpt-4o", calls: 100, total_tokens: 40000, total_cost: 1.0}, {model: "claude-3", calls: 50, total_tokens: 10000, total_cost: 0.25}]`
**When** se renderiza
**Then** el gráfico tiene 2 barras
**And** la tabla tiene 2 filas ordenadas por coste descendente

### Requirement: StatsDashboard SHALL display daily time series

StatsDashboard SHALL mostrar series temporales de llamadas y coste por día con un selector de rango.

**Given** el endpoint `/api/stats/llm/by-day?days=30` devuelve datos
**When** se renderiza StatsDashboard
**Then** muestra un gráfico de líneas con calls/día
**And** un gráfico de líneas con coste/día
**And** un selector de rango (7d, 30d)

#### Scenario: Serie temporal de 7 días
**Given** by-day devuelve 7 puntos de datos
**When** se selecciona "7d"
**Then** el gráfico muestra 7 puntos
**And** el eje X muestra fechas en formato "DD MMM"

#### Scenario: Serie temporal de 30 días
**Given** by-day devuelve 30 puntos de datos
**When** se selecciona "30d"
**Then** el gráfico muestra 30 puntos

### Requirement: StatsDashboard SHALL display tool call frequency

StatsDashboard SHALL mostrar la frecuencia de uso de las herramientas mediante un gráfico de tarta y una tabla.

**Given** el endpoint `/api/stats/llm/tools` devuelve datos
**When** se renderiza StatsDashboard
**Then** muestra un gráfico de tarta (doughnut) con las tools más llamadas
**And** una tabla con tool name y count

#### Scenario: Tools con datos
**Given** tools devuelve `[{tool: "get_weather", count: 45}, {tool: "search_web", count: 30}, {tool: "calendar", count: 15}]`
**When** se renderiza
**Then** el doughnut tiene 3 segmentos
**And** la tabla tiene 3 filas

### Requirement: StatsDashboard SHALL display database table sizes

StatsDashboard SHALL mostrar una tabla con el número de filas de cada tabla ordenada de mayor a menor.

**Given** el endpoint `/api/stats/db/sizes` devuelve datos
**When** se renderiza StatsDashboard
**Then** muestra una tabla con nombre de tabla y row count
**And** las tablas se ordenan por row count descendente

#### Scenario: DB sizes con datos
**Given** db/sizes devuelve `[{table: "messages", rows: 1500}, {table: "events", rows: 200}, ...]`
**When** se renderiza
**Then** messages aparece primero (mayor row count)

### Requirement: StatsDashboard SHALL allow CSV export

StatsDashboard SHALL permitir descargar un CSV con todos los datos de `llm_requests`.

**Given** la página StatsDashboard
**When** el usuario hace clic en "Export CSV"
**Then** se descarga un archivo CSV con todos los datos de llm_requests

#### Scenario: Export exitoso
**Given** hay datos en llm_requests
**When** el usuario clica "Export CSV"
**Then** el navegador descarga `valet-llm-requests.csv`
**And** el contenido es un CSV válido con cabeceras

### Requirement: StatsDashboard SHALL allow configuring retention days from UI

StatsDashboard SHALL permitir configurar `stats_retention_days` desde la UI, cargando y guardando el valor vía la API de retención.

**Given** la página StatsDashboard
**When** el usuario ve la sección de configuración
**Then** muestra un control para ajustar `stats_retention_days` (número, min 7, max 365)
**And** el valor actual se carga desde `GET /api/stats/retention`
**And** al guardar se llama a `PUT /api/stats/retention` con el nuevo valor

#### Scenario: Cargar retention actual
**Given** `GET /api/stats/retention` devuelve `{"days": 30}`
**When** se renderiza la sección de configuración
**Then** el input muestra "30"

#### Scenario: Guardar retention
**Given** el usuario cambia el valor a 45
**When** hace clic en "Guardar"
**Then** se llama `PUT /api/stats/retention` con `{"days": 45}`
**And** se muestra un mensaje de confirmación

### Requirement: StatsDashboard SHALL handle loading and error states

StatsDashboard SHALL mostrar estados de carga y de error por sección sin romper el resto del dashboard.

#### Scenario: Loading state
**Given** la página StatsDashboard se está cargando
**When** los endpoints aún no han respondido
**Then** muestra un spinner o skeleton en cada sección

#### Scenario: Error state
**Given** un endpoint devuelve error 500
**When** se renderiza StatsDashboard
**Then** muestra un mensaje de error en la sección afectada
**And** el resto de secciones siguen funcionando

### Requirement: StatsDashboard dialog SHALL display LLM usage summary

StatsDashboard SHALL display LLM usage summary in a dialog modal instead of a page.

**Given** el usuario abre el diálogo Stats desde el header
**When** el modal StatsDashboard se renderiza
**Then** muestra una tarjeta de resumen global con:
- Total de llamadas al modelo
- Total de tokens (prompt + completion)
- Tokens cacheados
- Tokens de razonamiento
- Coste total en USD
- Tasa de error (%)
- Promedio de duración (si hay datos)

#### Scenario: Resumen con datos
**Given** el endpoint `/api/stats/llm/summary` devuelve `{total_calls: 150, total_tokens: 50000, total_cached_tokens: 5000, total_reasoning_tokens: 3000, total_cost: 1.25, total_errors: 3, avg_duration_ms: 1200}`
**When** se abre el diálogo Stats
**Then** la tarjeta de resumen muestra "150", "50,000", "5,000 cached", "3,000 reasoning", "$1.25", "2.0%", "1.2s"

#### Scenario: Resumen sin datos (estado vacío)
**Given** el endpoint devuelve `{total_calls: 0, total_cost: 0, ...}`
**When** se abre el diálogo Stats
**Then** muestra "No data yet" o valores en cero
**And** no muestra gráficos vacíos

### Requirement: Stats opens as Modal dialog

Stats SHALL open as a Modal dialog when clicking the bar chart icon in the header, instead of navigating to the /stats route.

**Given** el usuario está en la vista de chat principal
**When** hace clic en el icono de gráfico de barras del header
**Then** se abre un Modal titulado "📊 Stats" con el dashboard de stats
**And** la vista de chat permanece visible detrás del modal

#### Scenario: Stats dialog closes
**Given** el diálogo Stats está abierto
**When** el usuario hace clic en el botón de cerrar (X) o fuera del modal
**Then** el modal se cierra
**And** la vista de chat vuelve a estar completamente visible

#### Scenario: /stats route renders chat view
**Given** el usuario navega a `/stats`
**Then** la app renderiza la vista de chat por defecto (no el dashboard de stats)

### Requirement: SettingsDialog SHALL display Perfil tab

SettingsDialog SHALL display a "Perfil" tab with a form to edit user profile (name and avatar URL).

**Given** el SettingsDialog está abierto en la tab "Perfil"
**When** se renderiza
**Then** muestra un formulario con campos "Nombre" (Input) y "Avatar URL" (Input)
**And** los valores iniciales se cargan desde `useProfile()`

#### Scenario: Perfil se guarda correctamente
**Given** el usuario ha modificado "Nombre" a "Juan"
**When** hace clic en "Guardar"
**Then** se llama a `profile.updateProfile({ name: "Juan" })`
**And** se muestra mensaje "Perfil actualizado"
**And** el diálogo se cierra

#### Scenario: Error al guardar perfil
**Given** `updateProfile` lanza un error
**When** el usuario guarda
**Then** se muestra mensaje "Error al actualizar perfil"
**And** el diálogo permanece abierto

### Requirement: SettingsDialog SHALL display Interfaz tab

SettingsDialog SHALL display an "Interfaz" tab with font size, context window, and page size controls.

**Given** el SettingsDialog está abierto en la tab "Interfaz"
**When** se renderiza
**Then** muestra:
- "Tamaño de fuente" (InputNumber, min 12, max 24)
- "Ventana de contexto (tokens)" (InputNumber, min 1000, max 100000)
- "Tamaño de página" (InputNumber, min 10, max 100)

#### Scenario: Interfaz se guarda correctamente
**Given** el usuario cambia font_size a 18
**When** hace clic en "Guardar"
**Then** se llama a `updateSettings` con los valores actualizados
**And** se muestra mensaje "Ajustes guardados"
**And** el diálogo se cierra

### Requirement: SettingsDialog SHALL display Prompt tab

SettingsDialog SHALL display a "Prompts" tab with three sub-tabs, one per editable prompt: System, Archivist and Collapse.

**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** muestra tres sub-pestañas: "System", "Archivist" y "Collapse"
**And** cada sub-pestaña muestra un TextArea de 10 filas con el valor actual de `system_prompt`, `archivist_prompt` y `collapse_prompt` respectivamente
**And** los valores se cargan desde `GET /settings`
**And** un botón "Guardar" persiste los tres valores vía `PUT /settings`

#### Scenario: Las tres sub-pestañas están presentes
**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** existen las sub-pestañas "System", "Archivist" y "Collapse"
**And** al hacer clic en cada una se muestra su TextArea correspondiente

#### Scenario: System prompt se carga desde la BD
**Given** `GET /settings` devuelve `system_prompt = "Eres Valet"`
**When** se abre la sub-pestaña "System"
**Then** el TextArea muestra "Eres Valet"

#### Scenario: Archivist prompt se carga desde la BD
**Given** `GET /settings` devuelve `archivist_prompt = "Eres un archivista"`
**When** se abre la sub-pestaña "Archivist"
**Then** el TextArea muestra "Eres un archivista"

#### Scenario: Collapse prompt se carga desde la BD
**Given** `GET /settings` devuelve `collapse_prompt = "Resume el texto"`
**When** se abre la sub-pestaña "Collapse"
**Then** el TextArea muestra "Resume el texto"

#### Scenario: Los tres prompts se guardan
**Given** el usuario edita los tres TextAreas
**When** hace clic en "Guardar"
**Then** `updateSettings` se llama con `{ system_prompt, archivist_prompt, collapse_prompt }`
**And** se muestra mensaje "Ajustes guardados"

#### Scenario: Prompt se guarda
**Given** el usuario escribe "Eres un asistente útil" en el TextArea "System"
**When** hace clic en "Guardar"
**Then** `updateSettings` se llama con `{ system_prompt: "Eres un asistente útil", ... }`

#### Scenario: Editar un prompt no borra los otros
**Given** el usuario modifica solo el TextArea "Archivist"
**When** hace clic en "Guardar"
**Then** `system_prompt` y `collapse_prompt` se envían con sus valores actuales sin cambios

### Requirement: SettingsDialog SHALL display API Keys tab

SettingsDialog SHALL display an "API Keys" tab with password fields for API keys.

**Given** el SettingsDialog está abierto en la tab "API Keys"
**When** se renderiza
**Then** muestra tres campos Input.Password:
- "OpenWeatherMap API Key"
- "Google Places API Key"
- "Brave Search API Key"

#### Scenario: API Keys se guardan
**Given** el usuario introduce una API key de OpenWeatherMap
**When** guarda
**Then** `updateSettings` se llama con la API key incluida

### Requirement: SettingsDialog SHALL unify profile and settings buttons

SettingsDialog SHALL open when clicking the settings icon in the header, replacing the separate profile and settings drawers.

**Given** el header tiene un botón con icono `SettingOutlined`
**When** el usuario hace clic en él
**Then** se abre un Modal titulado "⚙️ Settings" con tabs: Perfil, Interfaz, Prompts, API Keys
**And** el botón `UserOutlined` ya no existe en el header

#### Scenario: Dialog closes
**Given** el SettingsDialog está abierto
**When** el usuario hace clic en X o fuera del modal
**Then** el modal se cierra

#### Scenario: Modal con tabs funcionales
**Given** el SettingsDialog está abierto
**When** el usuario hace clic en la tab "API Keys"
**Then** se muestra el contenido de API Keys
**And** las otras tabs no están visibles

### Requirement: SettingsDialog SHALL handle loading and error states

SettingsDialog SHALL permitir restaurar los valores por defecto de la interfaz mostrando un mensaje de confirmación.

#### Scenario: Restaurar valores por defecto
**Given** el usuario ha modificado valores en Interfaz
**When** hace clic en "Restaurar valores por defecto"
**Then** todos los campos vuelven a sus valores default
**And** se muestra mensaje "Valores por defecto restaurados"

### Requirement: Message TypeScript interface SHALL include location

**Given** el tipo `Message` en `frontend/src/types/index.ts`  
**When** se renderiza un mensaje  
**Then** `Message` SHALL incluir `location?: string | null`

#### Scenario: Message incluye location opcional
**Given** el tipo `Message` en `frontend/src/types/index.ts`
**When** se define un mensaje
**Then** `Message` incluye `location?: string | null`

### Requirement: ChatView SHALL show date separators between message groups

**Given** una lista de mensajes ordenados cronológicamente  
**When** se renderizan en ChatView  
**Then** entre grupos de mensajes del mismo día calendario SHALL mostrar un `DateSeparator`

#### Scenario: Misma fecha → sin separador entre mensajes consecutivos
**Given** dos mensajes con `created_at` del mismo día (ej. "2026-09-27T10:00:00Z" y "2026-09-27T11:00:00Z")  
**When** se renderizan  
**Then** NO hay separador entre ellos

#### Scenario: Fecha diferente → separador entre grupos
**Given** un mensaje con fecha "2026-09-26T23:00:00Z" y otro con "2026-09-27T01:00:00Z"  
**When** se renderizan  
**Then** aparece un `DateSeparator` con formato legible entre ambos mensajes

#### Scenario: DateSeparator muestra "Hoy" para fecha actual
**Given** la fecha actual es 2026-09-27  
**Given** un mensaje con fecha "2026-09-27T10:00:00Z"  
**When** se renderiza  
**Then** el separador muestra "Hoy"

#### Scenario: DateSeparator muestra "Ayer" para día anterior
**Given** la fecha actual es 2026-09-27  
**Given** un mensaje con fecha "2026-09-26T10:00:00Z"  
**When** se renderiza  
**Then** el separador muestra "Ayer"

#### Scenario: DateSeparator muestra fecha completa para días más antiguos
**Given** un mensaje con fecha "2026-09-25T10:00:00Z"  
**When** se renderiza  
**Then** el separador muestra "25 sept 2026"

### Requirement: MessageBubble SHALL show timestamp inline and non-intrusive

**Given** un mensaje renderizado  
**When** se visualiza  
**Then** dentro de la burbuja, junto al role label, SHALL aparecer la hora en formato `HH:mm`

#### Scenario: Timestamp formatea created_at a hora local
**Given** `message.created_at = "2026-09-27T10:30:00Z"` (UTC)  
**When** se renderiza  
**Then** se muestra `10:30` (o la hora en timezone local del settings)

#### Scenario: Timestamp con location cuando existe
**Given** `message.created_at = "2026-09-27T10:30:00Z"` y `message.location = "Silla, Valencia, España"`  
**When** se renderiza  
**Then** se muestra `10:30 · 📍 Silla` (truncado a ciudad/pueblo)

#### Scenario: Timestamp sin location
**Given** `message.created_at = "2026-09-27T10:30:00Z"` y `message.location = null`  
**When** se renderiza  
**Then** se muestra solo `10:30`

#### Scenario: Mensaje streaming no muestra timestamp
**Given** mensaje con `id = "streaming"`  
**When** se renderiza  
**Then** NO se muestra timestamp ni location

### Requirement: Message TypeScript interface SHALL include tools_used

**Given** el tipo `Message` en `frontend/src/types/index.ts`  
**When** se renderiza un mensaje  
**Then** `Message` SHALL incluir `tools_used?: string`

#### Scenario: Message incluye tools_used opcional
**Given** el tipo `Message` en `frontend/src/types/index.ts`
**When** se define un mensaje
**Then** `Message` incluye `tools_used?: string`

### Requirement: MessageBubble SHALL display metadata as 3 separate lines

**Given** un mensaje renderizado  
**When** se visualiza  
**Then** los metadatos SHALL mostrarse en 3 líneas separadas debajo del contenido:
- **Línea 1**: `role · HH:mm` (siempre, excepto streaming)
- **Línea 2**: `📍 location` (solo si `location` no es null)
- **Línea 3**: `🔧 tools_used` (solo si `tools_used` no es null/undefined)
**And** NO SHALL parsear el contenido del mensaje para extraer herramientas

#### Scenario: Las 3 líneas con todos los metadatos presentes
**Given** `message.role = "assistant"`, `message.created_at = "2026-09-27T10:30:00Z"`, `message.location = "Carrer d'Alacant, València, Comunitat Valenciana, España"`, `message.tools_used = "calendar::get_events, weather::get_weather"`  
**When** se renderiza  
**Then** la línea 1 SHALL contener `assistant · 10:30`  
**And** la línea 2 SHALL contener `📍 Carrer d'Alacant, València, Comunitat Valenciana, España`  
**And** la línea 3 SHALL contener `🔧 calendar::get_events, weather::get_weather`

#### Scenario: Solo role y hora (sin location, sin tools)
**Given** `message.location = null`, `message.tools_used = null`  
**When** se renderiza  
**Then** solo SHALL mostrarse la línea 1 con `role · HH:mm`  
**And** NO SHALL mostrarse línea de ubicación  
**And** NO SHALL mostrarse línea de herramientas

#### Scenario: Role y hora + location (sin tools)
**Given** `message.location = "València"`, `message.tools_used = null`  
**When** se renderiza  
**Then** línea 1: `role · HH:mm`  
**And** línea 2: `📍 València`  
**And** NO SHALL mostrarse línea de herramientas

#### Scenario: Role y hora + tools (sin location)
**Given** `message.location = null`, `message.tools_used = "weather::get_weather"`  
**When** se renderiza  
**Then** línea 1: `role · HH:mm`  
**And** NO SHALL mostrarse línea de ubicación  
**And** línea 3: `🔧 weather::get_weather`

#### Scenario: tools_used con contadores usa coma como separador
**Given** `message.tools_used = "(3) calendar::get_events, weather::get_weather"`  
**When** se renderiza  
**Then** línea 3 SHALL contener `🔧 (3) calendar::get_events, weather::get_weather`

#### Scenario: Mensaje streaming no muestra metadatos
**Given** mensaje con `id = "streaming"`  
**When** se renderiza  
**Then** NO SHALL mostrarse ninguna línea de metadatos

### Requirement: Location SHALL display consistently (not only on reload)

**Given** un mensaje con `location`  
**When** se renderiza  
**Then** la ubicación SHALL mostrarse siempre que `location` no sea null  
**And** SHALL mostrarse en su propia línea (no inline con la hora)

#### Scenario: La ubicación se muestra en su propia línea
**Given** un mensaje con `location`
**When** se renderiza
**Then** la ubicación se muestra siempre que `location` no sea null
**And** se muestra en su propia línea, no inline con la hora

### Requirement: useMainChat SHALL NOT append tool footer to content

**Given** el hook `useMainChat`  
**When** se completa el streaming (`onDone`)  
**Then** NO SHALL concatenar `\n\n---\n🔧 ...` al contenido del mensaje  
**And** el contenido del mensaje SHALL ser exactamente el texto del assistant sin metadatos de herramientas

#### Scenario: El contenido no incluye footer de herramientas
**Given** el hook `useMainChat`
**When** se completa el streaming (`onDone`)
**Then** el contenido del mensaje no concatena el footer `🔧 ...`
**And** el contenido es exactamente el texto del assistant

### Requirement: MessageBubble SHALL render the Valet logo as assistant avatar

Los mensajes con rol `assistant` SHALL mostrar el logotipo de Valet (SVG vectorial)
como avatar a la izquierda de la burbuja, en lugar del icono genérico
`RobotOutlined` de Ant Design.

**Given** un mensaje con `role = "assistant"`
**When** `MessageBubble` se renderiza
**Then** SHALL aparecer el logo de Valet como avatar a la izquierda de la burbuja
**And** NO SHALL renderizarse el icono `RobotOutlined`

#### Scenario: Mensaje del asistente muestra el logo de Valet
**Given** `message.role = "assistant"` y `message.content = "Hola, soy Valet"`
**When** se renderiza `MessageBubble`
**Then** existe una imagen (`<img>`) cuyo `src` apunta al asset del logo de Valet
**And** esa imagen tiene un `alt` no vacío
**And** NO existe un elemento con la clase del icono robot de Ant Design

#### Scenario: Mensaje de streaming usa el mismo avatar
**Given** `message.id = "streaming"` y `message.role = "assistant"`
**When** se renderiza `MessageBubble`
**Then** el avatar SHALL ser también el logo de Valet
**And** NO SHALL renderizarse el icono `RobotOutlined`

#### Scenario: El rol system y el rol tool conservan su icono
**Given** mensajes con `role = "system"` y `role = "tool"`
**When** se renderizan
**Then** SHALL mantener sus iconos actuales (`InfoCircleOutlined`, `CodeOutlined`)
**And** NO SHALL mostrarse el logo de Valet

### Requirement: MessageBubble SHALL render the user profile avatar in user messages

Los mensajes con rol `user` SHALL mostrar como avatar la imagen del perfil recibida
en la prop `userAvatarUrl`, a la derecha de la burbuja. Si `userAvatarUrl` es
`null`, `undefined` o cadena vacía, SHALL mantener el icono `UserOutlined` actual.

**Given** un mensaje con `role = "user"`
**When** `MessageBubble` se renderiza
**Then** SHALL usar `userAvatarUrl` como avatar si está disponible
**And** SHALL degradar a `UserOutlined` si no lo está

#### Scenario: Usuario con avatar configurado
**Given** `message.role = "user"` y `userAvatarUrl = "https://example.com/me.png"`
**When** se renderiza `MessageBubble`
**Then** existe una imagen con `src = "https://example.com/me.png"` y `alt` no vacío
**And** NO existe el icono `anticon-user`

#### Scenario: Usuario sin avatar configurado (degradación)
**Given** `message.role = "user"` y `userAvatarUrl = null`
**When** se renderiza `MessageBubble`
**Then** SHALL renderizarse el icono `UserOutlined`
**And** NO SHALL renderizarse ninguna imagen de avatar de usuario

#### Scenario: El avatar del usuario NO se usa en otros roles
**Given** `userAvatarUrl = "https://example.com/me.png"`
**And** mensajes con `role = "assistant"`, `role = "system"` y `role = "tool"`
**When** se renderizan
**Then** NO SHALL usarse `userAvatarUrl` en ninguno de ellos

### Requirement: ChatView SHALL forward the user avatar URL to message bubbles

`ChatView` SHALL aceptar una prop opcional `userAvatarUrl?: string | null` y
reenviarla a cada `MessageBubble` que renderice, incluido el mensaje sintético de
streaming.

**Given** un `ChatView` con `userAvatarUrl` disponible
**When** se renderiza
**Then** cada `MessageBubble` SHALL recibir esa misma URL
**And** los mensajes con `role = "user"` SHALL mostrar ese avatar

#### Scenario: Propagación de la URL a las burbujas
**Given** `messages` con un mensaje de rol `user` y `userAvatarUrl = "https://example.com/me.png"`
**When** se renderiza `ChatView`
**Then** la burbuja del mensaje de usuario SHALL mostrar una imagen con ese `src`

#### Scenario: Prop ausente no rompe el render
**Given** un `ChatView` renderizado sin la prop `userAvatarUrl`
**When** se renderiza
**Then** los mensajes con `role = "user"` SHALL mostrar `UserOutlined`
**And** NO SHALL lanzarse ningún error

### Requirement: ProfileProvider SHALL provide a single shared profile to the whole app

El perfil del usuario SHALL obtenerse una sola vez y compartirse mediante un
contexto React (`ProfileProvider` + `useProfileContext()`), de modo que
`SettingsDialog` y el chat consuman la misma instancia y no haya copias
desincronizadas.

**Given** la app envuelta en `<ProfileProvider>`
**When** un consumidor llama a `useProfileContext()`
**Then** recibe `{ profile, loading, error, updateProfile }`
**And** solo SHALL realizarse una petición `GET /api/profile` en toda la app

#### Scenario: Uso fuera del provider falla explícitamente
**Given** un componente que llama a `useProfileContext()` sin un `<ProfileProvider>` ancestro
**When** se renderiza
**Then** SHALL lanzarse un error indicando que falta el provider

#### Scenario: Editar el avatar en Ajustes actualiza el chat sin recargar
**Given** el usuario cambia "Avatar URL" en la pestaña Perfil y pulsa guardar
**When** `updateProfile` resuelve con el perfil actualizado
**Then** el contexto SHALL exponer el nuevo `avatar_url`
**And** los mensajes con `role = "user"` del chat SHALL mostrar el nuevo avatar sin recargar la página

#### Scenario: Proveedor único
**Given** `SettingsDialog` y `AppLayout` renderizados bajo el mismo `<ProfileProvider>`
**When** ambos consumen el perfil
**Then** SHALL compartir la misma instancia de perfil
**And** NO SHALL duplicarse la petición a `GET /api/profile`

### Requirement: MessageBubble SHALL fall back to UserOutlined when the user avatar fails to load

Cuando la imagen del avatar del perfil dispara un evento `error`, `MessageBubble`
SHALL dejar de renderizar la imagen y SHALL mostrar el icono `UserOutlined` en su
lugar, sin recargar la página ni perder el resto del mensaje.

**Given** un mensaje con `role = "user"` y un `userAvatarUrl` que no carga
**When** la imagen del avatar dispara un evento `error`
**Then** SHALL renderizarse el icono `UserOutlined`
**And** NO SHALL quedar visible la imagen rota

#### Scenario: URL rota degrada a UserOutlined
**Given** `message.role = "user"` y `userAvatarUrl = "https://example.com/roto.png"`
**When** se renderiza `MessageBubble` y la imagen del avatar dispara `error`
**Then** existe el icono `.anticon-user`
**And** NO existe ninguna imagen de avatar del usuario

#### Scenario: Una URL válida no degrada
**Given** `message.role = "user"` y `userAvatarUrl = "https://example.com/me.png"`
**When** se renderiza `MessageBubble` sin disparar ningún `error`
**Then** SHALL mostrarse la imagen del avatar
**And** NO SHALL mostrarse el icono `UserOutlined`

#### Scenario: Cambiar de URL recupera la imagen
**Given** un `UserAvatar` que ha degradado a `UserOutlined` por un error de carga
**When** la prop `src` cambia a una URL nueva
**Then** SHALL volver a intentarse la carga y SHALL mostrarse la imagen

### Requirement: UserAvatar SHALL avoid referrer leakage and load lazily

La imagen del avatar del perfil SHALL declarar `referrerPolicy="no-referrer"` y
`loading="lazy"` en el elemento `img`.

**Given** un `UserAvatar` con un `src` no vacío
**When** se renderiza
**Then** el elemento `img` SHALL tener `referrerPolicy="no-referrer"`
**And** el elemento `img` SHALL tener `loading="lazy"`
**And** SHALL tener un `alt` no vacío

#### Scenario: Atributos de privacidad y carga diferida
**Given** `<UserAvatar src="https://example.com/me.png" />`
**When** se renderiza
**Then** el `img` resultante tiene `referrerpolicy="no-referrer"` y `loading="lazy"`

#### Scenario: Sin src no se renderiza ninguna imagen
**Given** `<UserAvatar src={null} />`
**When** se renderiza
**Then** SHALL mostrarse `UserOutlined`
**And** NO SHALL renderizarse ningún `img`

#### Scenario: Un src en blanco se trata como ausente
**Given** `<UserAvatar src="   " />` (solo espacios)
**When** se renderiza
**Then** SHALL tratarse como si no hubiera avatar y SHALL mostrarse `UserOutlined`
**And** NO SHALL renderizarse ningún `img`

### Requirement: SettingsDialog SHALL validate the avatar URL scheme

El formulario de la pestaña Perfil SHALL aceptar en "Avatar URL" únicamente un valor
vacío, una ruta relativa del propio host (que empiece por una única `/`), o una URL
absoluta con esquema `http` o `https`. Cualquier otro esquema (por ejemplo
`javascript:`, `data:` o `file:`), una URL relativa al protocolo (que empiece por
`//`, porque apunta a un host externo) y cualquier valor que contenga caracteres de
control o espacios embebidos SHALL mostrar un error de validación y SHALL impedir el
guardado. El valor SHALL persistirse recortado de espacios al principio y al final.

**Given** el formulario de Perfil con un valor en "Avatar URL"
**When** el usuario pulsa guardar
**Then** si el esquema no es `http`/`https` y no es una ruta relativa, SHALL mostrarse
un error de validación
**And** NO SHALL llamarse a `updateProfile`

#### Scenario: Esquema no permitido muestra error y no guarda
**Given** el usuario escribe `javascript:alert(1)` en "Avatar URL"
**When** pulsa guardar
**Then** SHALL mostrarse un error de validación en ese campo
**And** NO SHALL llamarse a `updateProfile`

#### Scenario: URL https válida se guarda
**Given** el usuario escribe `https://example.com/me.png` en "Avatar URL"
**When** pulsa guardar
**Then** SHALL llamarse a `updateProfile` con esa URL
**And** NO SHALL mostrarse ningún error de validación

#### Scenario: Valor vacío sigue siendo válido
**Given** el usuario deja "Avatar URL" vacío
**When** pulsa guardar
**Then** SHALL llamarse a `updateProfile`
**And** NO SHALL mostrarse ningún error de validación

#### Scenario: Ruta relativa válida se guarda
**Given** el usuario escribe `/avatars/me.png` en "Avatar URL"
**When** pulsa guardar
**Then** SHALL llamarse a `updateProfile` con esa ruta
**And** NO SHALL mostrarse ningún error de validación

#### Scenario: El valor se persiste recortado
**Given** el usuario escribe `"  https://example.com/me.png  "` con espacios al principio y al final en "Avatar URL"
**When** pulsa guardar
**Then** SHALL llamarse a `updateProfile` con `avatar_url = "https://example.com/me.png"`, sin los espacios
**And** NO SHALL mostrarse ningún error de validación

#### Scenario: Una URL relativa al protocolo se rechaza
**Given** el usuario escribe `//evil.com/a.png` en "Avatar URL"
**When** pulsa guardar
**Then** SHALL mostrarse un error de validación en ese campo
**And** NO SHALL llamarse a `updateProfile`

#### Scenario: Caracteres de control embebidos se rechazan
**Given** el usuario escribe un valor que contiene un tabulador embebido, como "java<TAB>script:alert(1)"
**When** pulsa guardar
**Then** SHALL mostrarse un error de validación en ese campo
**And** NO SHALL llamarse a `updateProfile`

### Requirement: SettingsDialog SHALL display a Memoria tab with the four memory knobs

SettingsDialog SHALL display a "Memoria" tab that agrupa en un `Tabs` anidado dos sub-pestañas: «Episódica» y «Persistente». La sub-pestaña «Episódica» SHALL mostrar cuatro campos numéricos, uno por mando de memoria: `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES`. La sub-pestaña «Persistente» SHALL mostrar el panel de memoria persistente descrito en la spec `persistent-memory-ui`. Los valores de la sub-pestaña «Episódica» SHALL cargarse de `GET /settings` y guardarse con `PUT /settings`, sin rutas nuevas de API. Al ser ajustables en caliente, un cambio guardado SHALL surtir efecto sin reiniciar.

**Given** el SettingsDialog está abierto en la pestaña "Memoria" con la sub-pestaña "Episódica" activa  
**When** se renderiza  
**Then** muestra cuatro campos numéricos: `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES`  
**And** cada campo muestra el valor actual cargado de `GET /settings`  
**And** un botón "Guardar" persiste los cuatro valores vía `PUT /settings`

#### Scenario: Los cuatro campos están presentes
**Given** el SettingsDialog está abierto en la pestaña "Memoria" con la sub-pestaña "Episódica" activa  
**When** se renderiza  
**Then** existen los campos `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES`

#### Scenario: Los valores se cargan desde la BD
**Given** `GET /settings` devuelve `MEMORY_HALF_LIFE_DAYS = 90` y `RAG_BUDGET_TOKENS = 800`  
**When** se abre la sub-pestaña "Episódica"  
**Then** el campo `MEMORY_HALF_LIFE_DAYS` muestra `90`  
**And** el campo `RAG_BUDGET_TOKENS` muestra `800`

#### Scenario: Los cuatro mandos se guardan
**Given** el usuario edita los cuatro campos  
**When** hace clic en "Guardar"  
**Then** `updateSettings` se llama con `{ MEMORY_HALF_LIFE_DAYS, SIMILARITY_THRESHOLD, RAG_BUDGET_TOKENS, MEMORY_KNN_CANDIDATES }`  
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Editar un mando no borra los otros
**Given** el usuario modifica solo `SIMILARITY_THRESHOLD`  
**When** hace clic en "Guardar"  
**Then** los otros tres mandos se envían con sus valores actuales sin cambios

### Requirement: SettingsDialog SHALL display a Generación tab with the generation knobs

SettingsDialog SHALL display a "Generación" tab with four blocks —Chat, Colapso, Fichas y
Consolidación—, y dentro de cada bloque tres campos: temperatura (numérico), razonamiento
(selector) y tokens máximos (numérico). El selector de razonamiento SHALL ofrecer al menos
`default` (no enviar), `off`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`. Los valores SHALL
cargarse de `GET /settings` y guardarse con `PUT /settings`, sin rutas nuevas de API. Al ajustarse
en caliente, un cambio guardado SHALL surtir efecto sin reiniciar.

**Given** el SettingsDialog está abierto en la tab "Generación"  
**When** se renderiza  
**Then** muestra cuatro bloques, uno por rol  
**And** cada bloque muestra temperatura, razonamiento y tokens máximos  
**And** cada campo muestra el valor actual cargado de `GET /settings`  
**And** un botón "Guardar" persiste las doce claves vía `PUT /settings`

#### Scenario: La pestaña muestra los cuatro roles y sus tres campos
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se renderiza  
**Then** existen los bloques Chat, Colapso, Fichas y Consolidación  
**And** cada bloque tiene temperatura, razonamiento y tokens máximos

#### Scenario: Los valores se cargan desde la BD
**Given** `GET /settings` devuelve `GENERATION_CHAT_TEMPERATURE = 0.7` y `GENERATION_SEMANTIC_REASONING = low`  
**When** se abre la tab "Generación"  
**Then** el campo de temperatura del chat muestra `0.7`  
**And** el selector de razonamiento de consolidación muestra `low`

#### Scenario: El selector de razonamiento ofrece las opciones esperadas
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se abre el selector de razonamiento de cualquier rol  
**Then** ofrece `default`, `off`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`

#### Scenario: Guardar envía las doce claves y muestra confirmación
**Given** el usuario edita la temperatura del chat  
**When** hace clic en "Guardar"  
**Then** `updateSettings` se llama con las doce claves, las editadas y las demás con su valor actual  
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Editar un campo no borra los demás
**Given** el usuario modifica solo los tokens máximos del consolidador  
**When** hace clic en "Guardar"  
**Then** las otras once claves se envían con sus valores actuales sin cambios

### Requirement: SettingsDialog SHALL organize the Generación roles in sub-tabs

Dentro de la pestaña «Generación», SettingsDialog SHALL mostrar un `Tabs` anidado con una sub-pestaña
por rol —**Chat**, **Colapso**, **Fichas** y **Consolidación**— siguiendo el patrón de la pestaña
«Prompts». La sub-pestaña «Consolidación» corresponde a las claves `GENERATION_SEMANTIC_*`. Cada
sub-pestaña SHALL contener los tres campos del rol (temperatura numérica, razonamiento por selector
y tokens máximos numéricos), manteniendo `name`/`label` iguales a la clave cruda. Las sub-pestañas
SHALL usar `forceRender` para que los doce campos permanezcan registrados en el formulario aunque su
sub-pestaña no esté activa, de modo que el guardado siga enviando las doce claves.

**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** se renderiza  
**Then** existe un `Tabs` anidado con las sub-pestañas Chat, Colapso, Fichas y Consolidación  
**And** la sub-pestaña activa muestra temperatura, razonamiento y tokens máximos de su rol

#### Scenario: Existen las cuatro sub-pestañas de rol
**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** se renderiza  
**Then** existen sub-pestañas con nombre Chat, Colapso, Fichas y Consolidación

#### Scenario: Cambiar de sub-pestaña muestra los campos del rol
**Given** el SettingsDialog abierto en "Generación" con la sub-pestaña "Chat" activa  
**When** el usuario selecciona la sub-pestaña "Colapso"  
**Then** los campos de colapso (`GENERATION_COLLAPSE_*`) quedan visibles  
**And** la sub-pestaña "Chat" deja de estar visible

#### Scenario: Los doce campos siguen registrados con forceRender
**Given** el usuario abrió la pestaña "Generación"  
**When** guarda sin haber abierto todas las sub-pestañas  
**Then** `updateSettings` recibe las doce claves `GENERATION_*` sin cambios en las no editadas

### Requirement: SettingsDialog SHALL fit all its top-level tabs without overflow

El `Modal` de SettingsDialog SHALL declarar un ancho de al menos **960 px** (se fija en 1000) para que sus siete pestañas superiores —Perfil, Interfaz, Prompts, API Keys, Memoria, Generación y Herramientas— quepan en una sola fila, sin que ninguna se oculte en el desplegable de desbordamiento de antd.

**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho del diálogo es de al menos 960 px  
**And** sus siete pestañas superiores son alcanzables

#### Scenario: El diálogo es más ancho que el mínimo
**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho declarado del `Modal` es mayor o igual a 960 px

#### Scenario: La pestaña Memoria persistente es alcanzable
**Given** el SettingsDialog abierto  
**When** el usuario abre la pestaña "Memoria" y selecciona la sub-pestaña "Persistente"  
**Then** se muestra el panel de memoria persistente

#### Scenario: Existen exactamente seis pestañas superiores
**Given** el SettingsDialog abierto  
**When** el usuario busca las pestañas superiores  
**Then** existen exactamente siete: Perfil, Interfaz, Prompts, API Keys, Memoria, Generación y Herramientas  
**And** NO existe una pestaña superior "Memoria persistente"

### Requirement: SettingsDialog SHALL organize the memory panels in sub-tabs

Dentro de la pestaña «Memoria», SettingsDialog SHALL mostrar un `Tabs` anidado con dos sub-pestañas —«Episódica» y «Persistente»— siguiendo el patrón de las pestañas «Prompts» y «Generación». El `Tabs` anidado SHALL ir envuelto en una región etiquetada (`<section aria-label="Tipo de memoria">`), de modo que las tecnologías de asistencia la anuncien; antd no propaga `aria-label` al `role="tablist"`, así que un `aria-label` sobre el propio `Tabs` sería cosmético. Ninguna de las dos sub-pestañas SHALL usar `forceRender`: antd monta cada panel de forma perezosa, de modo que la sub-pestaña «Persistente» —y con ella el `GET` del estado persistente— solo se monta cuando se selecciona.

**Given** el SettingsDialog abierto en la pestaña "Memoria"  
**When** se renderiza  
**Then** existe un `Tabs` anidado con las sub-pestañas Episódica y Persistente  
**And** la sub-pestaña activa por defecto es "Episódica"

#### Scenario: Existen las dos sub-pestañas
**Given** el SettingsDialog abierto en la pestaña "Memoria"  
**When** se renderiza  
**Then** existen sub-pestañas con nombre Episódica y Persistente

#### Scenario: El Tabs anidado está dentro de una región «Tipo de memoria»
**Given** el SettingsDialog abierto en la pestaña "Memoria"  
**When** se renderiza  
**Then** existe una región accesible con nombre "Tipo de memoria" que contiene las sub-pestañas Episódica y Persistente

#### Scenario: Cambiar de sub-pestaña activa el panel correspondiente
**Given** el SettingsDialog abierto en "Memoria" con la sub-pestaña "Episódica" activa  
**When** el usuario selecciona la sub-pestaña "Persistente"  
**Then** el panel de memoria persistente queda visible  
**And** la sub-pestaña "Episódica" deja de estar visible

#### Scenario: El panel persistente no se monta hasta seleccionar su sub-pestaña
**Given** el SettingsDialog abierto en la pestaña "Memoria" con la sub-pestaña "Episódica" activa  
**When** se renderiza sin seleccionar "Persistente"  
**Then** el panel de memoria persistente NO está montado

### Requirement: El chat SHALL pedir confirmación para herramientas destructivas

Ante un evento SSE `approval_required`, el chat SHALL mostrar la petición con el nombre de la
herramienta y el motivo, y SHALL ofrecer las acciones Permitir y Denegar. Al decidir SHALL llamar a
`POST /api/approval/{request_id}` y SHALL mantener abierto el stream para continuar la respuesta. El
estado pendiente SHALL limpiarse al recibir `approval_result`, `done` o `error`.

#### Scenario: La petición de aprobación se muestra

**Given** el chat recibiendo un evento `approval_required` con `request_id`, `tool_name` y `reason`
**When** llega el evento
**Then** se muestra un diálogo con el nombre de la herramienta y el motivo
**And** el diálogo ofrece Permitir y Denegar

#### Scenario: Aprobar reanuda la respuesta

**Given** un diálogo de aprobación visible
**When** el usuario pulsa Permitir
**Then** se llama a `POST /api/approval/{request_id}` con `approved: true`
**And** el diálogo se cierra
**And** el stream continúa y la respuesta se completa con `done`

#### Scenario: Denegar cierra el diálogo y continúa

**Given** un diálogo de aprobación visible
**When** el usuario pulsa Denegar
**Then** se llama a `POST /api/approval/{request_id}` con `approved: false`
**And** el diálogo se cierra
**And** el stream continúa hasta `done`

#### Scenario: El estado pendiente se limpia

**Given** una aprobación resuelta
**When** llega `approval_result`, `done` o `error`
**Then** no queda ningún diálogo de aprobación visible

### Requirement: SettingsDialog SHALL display Herramientas tab

SettingsDialog SHALL display a "Herramientas" top-level tab that lists the registered tools and lets
the user enable or disable each one. Al abrirse, el panel SHALL cargar la lista desde `GET /api/tools`
y SHALL mostrar cada tool con su nombre, su descripción y un `Switch` que refleja su campo `enabled`.
Al cambiar un `Switch`, SHALL llamar a `PUT /api/tools/{id}/toggle` y actualizar la fila con la tool
devuelta; si la llamada falla, SHALL restaurar el valor previo y mostrar un aviso de error.

**Given** el SettingsDialog abierto en la tab "Herramientas"
**When** el panel se monta
**Then** se llama a `GET /api/tools`
**And** se listan todas las tools con nombre, descripción y un `Switch` con su estado `enabled`

#### Scenario: La pestaña lista las tools con su estado
**Given** `GET /api/tools` devuelve una tool `weather` con `enabled: true`
**When** el usuario abre la pestaña "Herramientas"
**Then** la fila de `weather` muestra su nombre, su descripción y un `Switch` activado

#### Scenario: Deshabilitar una tool
**Given** la fila de la tool `weather` con el `Switch` activado
**When** el usuario apaga el `Switch`
**Then** se llama a `PUT /api/tools/weather/toggle` (con el `id` de la tool)
**And** el `Switch` queda apagado

#### Scenario: Habilitar de nuevo una tool
**Given** la fila de la tool `weather` con el `Switch` apagado
**When** el usuario enciende el `Switch`
**Then** se llama a `PUT /api/tools/weather/toggle`
**And** el `Switch` queda activado

#### Scenario: Error al cambiar el estado
**Given** la fila de la tool `weather` con el `Switch` activado
**When** el usuario apaga el `Switch` y la llamada a `PUT /api/tools/.../toggle` falla
**Then** el `Switch` vuelve a quedar activado
**And** se muestra un aviso de error

#### Scenario: Estado de carga
**Given** que `GET /api/tools` aún no ha respondido
**When** el usuario abre la pestaña "Herramientas"
**Then** se muestra un indicador de carga
**And** la lista de tools no se muestra hasta que llegan los datos
