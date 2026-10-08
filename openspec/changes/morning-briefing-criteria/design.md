# Design

## D1 — Por qué el texto y no el umbral

La frontera medida sobre el histórico real, **todas las configuraciones con `--repeat 3`** (base de datos de desarrollo):

| Configuración | Cobertura | tools/turno | Ahorro |
|---|---|---|---|
| `settings` (criteria de contenido) | 95,5 % (mín 93,9) | 9,69 | 28,3 % |
| Umbral focalizado (`agenda`/`pendientes`/`entorno` 0,05, `widgets` 0,08) | 100,0 % | 11,61 | 10,8 % |
| **Criteria con el saludo de apertura** (umbrales intactos) | **100,0 %** | **9,85** | **25,6 %** |

Bajar umbrales solo donde falla llega al 100 % pero cuesta **17,5 pp de ahorro** y dispara casi todas las skills (`entorno` 0,52→0,85, `widgets` 0,48→0,83). Reescribir el criterio llega al 100 % por **2,7 pp** y deja las activaciones casi intactas. Por eso este change mueve el texto.

Nota de método: las `criteria` **no son independientes**. Las seis preguntas viajan en una sola petición, así que editar una desplaza las respuestas de todas: al añadir el saludo, `recuerdos` (criterio intacto) pasó de 0,84 a 0,86. Por eso la comparación se hace con `--repeat 3` en ambos lados y se lee como rango, no con una sola pasada.

## D2 — Por qué estas cuatro skills y no otras

Los tres turnos usaron `calendar` (agenda), `tasks`/`reminders` (pendientes), `weather` (entorno) y `render_widget` (widgets). Nunca usaron `notes`/`unified_search` (recuerdos) ni búsqueda web: añadirles el saludo sería sobreactivación sin caso medido. Comparadas con las criteria de contenido (3 reps), las activaciones se mueven poco: `agenda` 0,61→0,62, `pendientes` 0,64→0,67, `entorno` 0,52→0,55, `widgets` 0,48→0,62 (el que quedaba fuera), `recuerdos` 0,84→0,86 y `web` 0,45→0,45. La ampliación es quirúrgica: el coste medido es el 2,7 pp de ahorro, no un desbordamiento de activaciones.

## D3 — Por qué el default compilado y no una fila en `settings`

Ninguna migración siembra las claves `SKILL_<ID>_CRITERIA_TRUE`/`_FALSE` (solo se siembran los `SKILL_<ID>_PROMPT`). El valor del catálogo es el **default y el respaldo** (requisito vigente «Las preguntas del clasificador SHALL ser editables desde settings con el valor compilado como respaldo»), así que aplica a instalaciones nuevas y a las existentes sin override. Sembrarlo en `settings` lo haría invisible y desplazaría el default fuera del código.

## D4 — El escenario que hay que ajustar

El requisito «La selección SHALL gobernar también el turno sin herramientas» ilustra hoy el turno vacío con «un saludo sin petición de acción». Con este change un saludo de apertura sí supera el umbral, así que ese ejemplo queda desmentido. La regla no cambia (sin superar el umbral, la selección es vacía); cambia el ejemplo a un turno conversacional sin acción que no sea un saludo de apertura («gracias, perfecto»). El escenario sigue probando exactamente lo mismo.

## D5 — Qué NO cambia y qué no promete este change

- **No enciende el enrutador**: `ROUTER_ENABLED=false` sigue, y sin encender, la conducta es idéntica a la de hoy.
- **No** toca umbrales, catálogo cerrado, herramientas, API, UI ni migraciones.
- **No promete** la puntuación que devuelva el clasificador: el requisito es sobre el **texto** de las criteria (determinista y testeable). El efecto medido (100,0 % en tres repeticiones) es la evidencia con la que se decidió, no una garantía de la spec.
- La decisión de **encender** el enrutador sigue siendo un change aparte (con su migración y su política de arranque).

## D6 — Fidelidad de la medida

La variante se midió primero como **fichero de overrides** (100,0 % de cobertura y 25,6 % de ahorro) y después **compilada** (100,0 % y 26,0 %): coinciden dentro del ruido del clasificador, así que lo que se entrega reproduce lo que se midió. Es la comprobación que hizo posible el change anterior (`route-eval-campaign-support`).
