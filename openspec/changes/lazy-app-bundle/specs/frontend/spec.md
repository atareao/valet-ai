## ADDED Requirements

### Requirement: El frontend SHALL cargar la aplicación autenticada de forma diferida

El bundle inicial del frontend —el que se descarga antes de resolver la sesión— SHALL excluir `antd`, `@ant-design/icons`, `react-router-dom`, las vistas autenticadas y las librerías de gráficas (`chart.js`) y markdown (`react-markdown`/`remark-gfm`). El módulo de la aplicación autenticada SHALL importarse dinámicamente cuando la sesión se resuelva, y las vistas que se abren desde el `AppLayout` (Agenda, Tareas, Stats y Ajustes) SHALL cargarse bajo demanda al abrirse, sin bloquear el bundle inicial. Si la carga de un módulo diferido falla, el frontend SHALL mostrar un aviso recuperable (con una acción de recarga) en lugar de desmontar la aplicación y dejar una pantalla en blanco.

#### Scenario: El bundle inicial no incluye antd ni la app autenticada
- **Given** una compilación de producción
- **When** se analiza el grafo de chunks iniciales (entrada HTML y sus importaciones estáticas)
- **Then** ningún chunk inicial pertenece a `antd`, `@ant-design/icons`, las gráficas, el markdown, leaflet ni la aplicación autenticada
- **And** el tamaño gzip total del grafo inicial no supera el presupuesto de la comprobación de build
- **And** existe un chunk diferido de la aplicación autenticada fuera de ese grafo

#### Scenario: La aplicación autenticada se carga bajo demanda
- **Given** un usuario con sesión resuelta
- **When** la aplicación se muestra
- **Then** el módulo de la aplicación autenticada se importa dinámicamente
- **And** mientras se carga se muestra el indicador de carga

#### Scenario: Las vistas del layout se cargan al abrirse
- **Given** la aplicación autenticada cargada
- **When** el usuario abre Agenda, Tareas, Stats o Ajustes
- **Then** el código de esa vista se carga bajo demanda y se muestra su contenido

#### Scenario: Un fallo al cargar un chunk diferido no deja la app en blanco
- **Given** la aplicación en ejecución con la sesión resuelta
- **When** falla la carga de un chunk diferido (por ejemplo, un 404 tras un despliegue)
- **Then** se muestra un aviso recuperable con una acción de recarga
- **And** la aplicación no queda desmontada en una pantalla en blanco
