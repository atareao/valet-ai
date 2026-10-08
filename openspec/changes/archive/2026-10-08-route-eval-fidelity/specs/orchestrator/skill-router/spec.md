## MODIFIED Requirements

### Requirement: El arnés de evaluación SHALL medir la cobertura de las herramientas realmente usadas

El arnés SHALL recorrer los turnos históricos cuyo mensaje de asistente registra herramientas usadas, ejecutar el enrutador sobre el mensaje del usuario y calcular la **cobertura**: la proporción de turnos en los que **toda** herramienta realmente usada figura en el conjunto que el enrutador habría expuesto. SHALL usar los **valores efectivos** de `settings` —criterios, umbrales y número de turnos de historial—, de modo que el bucle de ajuste sea «editar y medir». SHALL **publicar la configuración efectiva** con la que ha medido (las skills, sus umbrales y qué campos están sobrescritos), para que cada cifra sea atribuible. SHALL reportar además las activaciones por skill, la latencia p50 y p95, los tokens y el coste, y SHALL aceptar `--limit`, `--threshold`, `--model` y `--dry-run` (catálogo sin red).

El arnés SHALL medir además sobre el **estado que el clasificador recibe en producción**, y SHALL publicar la **palanca** que el enrutado mueve y la **varianza** de la medida:

- El contenido de cada mensaje SHALL ser `collapsed_content` cuando exista y `content` en caso contrario, igual que `orchestrator::agent`.
- La ventana SHALL ser el presupuesto de tokens de `settings.max_window_tokens` (por defecto `10000`) contado hacia atrás desde el turno, y SHALL NOT reiniciarse cuando dos turnos consecutivos no emparejen: un contexto que producción sí tendría no puede descartarse.
- SHALL instrumentar las **herramientas por turno** y el tamaño —en bytes y en tokens estimados con el estimador del propio proyecto— del bloque de definiciones expuesto frente al conjunto completo habilitado, y SHALL publicar el **ahorro** resultante. La medida SHALL tomarse de las **mismas definiciones** que viajan en la petición, no de una tabla de tamaños escrita a mano.
- `--repeat <N>` SHALL repetir la medición y publicar la cobertura de cada repetición junto al mínimo, la media y el máximo, de modo que la varianza se lea como rango.

#### Scenario: El arnés mide con los criterios vigentes
- **Given** un criterio sobrescrito en `settings`
- **When** se ejecuta el arnés
- **Then** las peticiones al clasificador usan el criterio sobrescrito
- **And** el informe declara qué campos están sobrescritos

#### Scenario: Un turno no cubierto se reporta
- **Given** un turno histórico que usó `tasks` y el enrutador no expuso `tasks`
- **When** se calcula la cobertura
- **Then** ese turno cuenta como no cubierto
- **And** el informe señala la skill `pendientes` como la que lo habría cubierto

#### Scenario: El modo sin red no llama al servicio
- **Given** el arnés en modo `--dry-run`
- **When** se ejecuta
- **Then** recorre el catálogo y el historial sin ninguna petición de red
- **And** declara la configuración efectiva que habría usado
- **And** declara la ventana que habría usado

#### Scenario: El estado del clasificador es el que recibe producción
- **Given** un turno histórico cuyo historial incluye un mensaje con `collapsed_content`
- **When** el arnés construye el estado para el clasificador
- **Then** ese mensaje viaja con el contenido colapsado, no con el crudo
- **And** la ventana la fija `max_window_tokens` contada hacia atrás desde el turno
- **And** el historial no se reinicia porque la pareja anterior no empareje

#### Scenario: El informe publica las herramientas por turno y el ahorro
- **Given** una ejecución medida
- **When** el arnés informa
- **Then** publica las herramientas medias por turno
- **And** publica los bytes y los tokens estimados del bloque expuesto frente al conjunto completo habilitado
- **And** publica el ahorro resultante

#### Scenario: La varianza se publica
- **Given** el arnés invocado con `--repeat 3`
- **When** mide
- **Then** repite la medición tres veces
- **And** publica la cobertura de cada repetición y el mínimo, la media y el máximo
