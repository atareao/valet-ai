# Design

## Context

El change `skill-router` dejó la infraestructura completa (catálogo, router, cliente de Jev, fragmentos, arnés, UI) con el enrutador **apagado**: la medición sobre el historial real dio **59,1 % de cobertura**, insuficiente para una feature que retira herramientas.

Las medidas posteriores, sobre los mismos **66 turnos** con herramientas registradas en `messages.tools_used` como verdad de referencia, son la base de este diseño:

| Variante medida | Cobertura a su mejor umbral | Ahorro del bloque |
|---|---|---|
| Catálogo actual (8 skills), criterios de tema, umbral 0,30 | 59,1 % | — |
| Mismo catálogo, criterios **en inglés** | peor en casi todo el rango | — |
| Mismo catálogo, criterios **operativos verbosos** | ~igual cobertura, **más activaciones** | menos |
| **6 dominios + criterios compactos + umbral por skill** | **97,0 %** | **28,1 %** |

Y la contraprueba que fija la prioridad de las palancas: **enrutar solo `render_widget`** da 98,5 % de cobertura con un ahorro del **12,8 %**; enrutarlo junto a los dominios **duplica** el ahorro a cambio de un único turno más, y ese turno es un saludo donde el modelo consultó el tiempo por su cuenta.

## Goals / Non-Goals

**Goals:**
- Llevar el enrutador a un punto **defendible con datos**: cobertura en el techo de la métrica y ahorro real del bloque de herramientas.
- Quitar la fricción que ha hecho de este ajuste una campaña manual: que las `criteria` se puedan **editar y medir sin recompilar**, sin perder el respaldo del valor por defecto.
- Que la configuración efectiva de cada medición sea **atribuible** (qué criterios y umbrales se usaron).

**Non-Goals:**
- **No** se tocan las herramientas: ni sus esquemas, ni sus operaciones, ni sus permisos. La fusión «1 skill = 1 tool» y el recorte de esquemas son el **cambio B**, y se medirán aparte.
- **No** se enciende el enrutador: sigue apagado por defecto.
- **No** se hace editable la **agrupación** de skills (sigue siendo un `enum` cerrado en código, con el test de integridad 13/13). Lo editable es el contenido: criterios y fragmentos.
- **No** se borran claves antiguas de `settings`.

## Decisions

### D1. Seis dominios amplios en lugar de ocho skills finas
Se fusionan `tareas`+`recordatorios` en `pendientes` y `notas`+`memoria` en `recuerdos`. El catálogo grueso gana en **todo** el rango de umbrales medido y con **tres preguntas menos**. La razón de fondo: pedirle al clasificador que distinga `tasks` de `reminders` es pedirle una decisión que el modelo principal toma mejor con ambos conjuntos de operaciones a la vista. El clasificador debe decidir **dominios**, no herramientas.

### D2. El core se reduce a `get_current_time` y `get_current_location`
238 bytes entre ambas. La segunda se mantiene por decisión de producto: saber dónde está el usuario es contexto básico de un asistente, y su coste es despreciable. `render_widget` sale del core: es el 28 % del bloque y su mejor argumento era que el prompt base lo **ordena** usar — lo que este cambio resuelve mudando esa guía a su fragmento (D5).

### D3. Las `criteria` son contenido: van a `settings`, con el valor compilado como fallback
Claves `SKILL_<ID>_QUESTION`, `SKILL_<ID>_CRITERIA_TRUE` y `SKILL_<ID>_CRITERIA_FALSE`, leídas en cada turno. El valor **compilado** sigue siendo el default y el fallback: una fila ausente, vacía o ilegible usa el del código, de modo que **una edición a medias no puede degradar el enrutado en silencio**. «Restaurar por defecto» es vaciar la fila. La agrupación, en cambio, se queda en código (con su test de integridad), porque es diseño y no un mando que se gire a menudo.
*Alternativa descartada:* sembrar los textos en la migración (idioma del proyecto para `system_prompt`). Duplicaría el mismo texto en dos sitios y haría que el valor por defecto dejara de ser único.

### D4. Criterios compactos, en español
Compactos porque a igual cobertura activan menos (4,26 frente a 4,80 skills por turno) y en español porque el inglés midió peor. Cada criterio enumera **condiciones observables en el mensaje** y un **negativo que nombra lo que el mensaje es en su lugar** («un aviso a una hora es recordatorio, no tarea»), en lugar de describir el tema de la skill.

### D5. La guía de widgets se muda a su fragmento
`render_widget` deja de estar siempre expuesto, así que una guía permanente en el prompt base que **ordena** usarlo dejaría al modelo con una instrucción incumplible. La migración retira el bloque **solo si está verbatim** (identificado por su encabezado) y lo deposita en `SKILL_WIDGETS_PROMPT`; si el usuario lo había editado, no se retira y la **regla anti-duplicado** del ensamblador impide que se inyecte dos veces. Con la skill inactiva no viajan ni el esquema ni la guía.

### D6. Umbral global 0,10 y override por skill para la capacidad discrecional
`ROUTER_THRESHOLD` = 0,10 (medido) y `ROUTER_THRESHOLD_<ID>` como override; sembrado `ROUTER_THRESHOLD_WIDGETS` = 0,20. El widget necesita umbral propio porque es **discrecional**: su probabilidad base (p50 0,21) está por encima de cualquier umbral bajo, de modo que con 0,10 quedaría expuesto casi siempre y no ahorraría nada; con 0,20 solo se pierde un turno, y es un saludo donde el widget lo propuso el modelo.

### D7. `ROUTER_HISTORY_TURNS` de 2 a 6
La medición se hizo con seis turnos de historial y la clase de fallo dominante eran **continuaciones** («ahora sí», «y el planito?»), donde la necesidad la arrastra la conversación y no el mensaje aislado. Producción recortaba a dos.

### D8. El arnés publica la configuración efectiva
Con las `criteria` en `settings`, el bucle de ajuste pasa a ser «editar → medir». Para que una cifra sea reproducible y atribuible, el informe declara las skills, los umbrales efectivos y qué criterios están sobrescritos.

## Risks / Trade-offs

- **La métrica tiene techo.** Los dos fallos del mejor punto son turnos donde el modelo usó herramientas por iniciativa propia ante un saludo: ningún clasificador los acierta. Perseguir el 100 % con esta métrica solo lleva a sobre-activar. El diseño se detiene en el techo, no por debajo.
- **66 turnos son pocos.** La diferencia entre 97,0 % y 100 % es **un** turno; el umbral no se afina más con esta muestra, y así se declara en vez de aparentar precisión.
- **El endpoint sigue en `alpha`** y el enrutador sigue **apagado**: si el servicio cambia o desaparece, el comportamiento actual no se ve afectado.
- **Una fusión de dominios pierde granularidad de ahorro**: `pendientes` expone dos herramientas cuando solo se necesitaba una. Es el precio de que el clasificador no tenga que acertar la distinción fina, y sale a cuenta según la medición.
- **Editar criterios en la UI es una superficie con aristas**: se mitiga con el fallback al valor compilado, la marca de sobrescrito en la API/UI y un aviso no bloqueante.
