# Proposal

## Why

La campaña midió los tres únicos turnos no cubiertos del histórico (66 turnos, base de datos real) y los tres son **saludos de apertura del día**: «Buenos días Bautista» (usó `calendar` y `tasks`), «Buenos días, Como tenemos hoy el día?» (`calendar`, hora, `weather` y `render_widget`) y «Buenos días» (`calendar`, hora, `reminders`, `weather` y `render_widget`). El asistente hace un **briefing matutino** al saludar, pero las `criteria` por defecto son de contenido y un saludo no menciona nada: el clasificador puntúa 0,05–0,09 y el enrutador suprimiría el briefing — una regresión de conducta, no una ganancia.

Medido con el arnés (`--repeat 3` en todos los casos, mismo histórico): con las criteria de contenido, 95,5 % de cobertura y 28,3 % de ahorro; bajando umbrales por skill, 100 % de cobertura pero solo 10,8 % de ahorro; con criteria que reconocen el saludo y umbrales intactos, **100,0 % de cobertura y 25,6 % de ahorro**. La palanca buena es el texto: recupera la cobertura por 2,7 pp de ahorro, frente a 17,5 pp del umbral.

## What Changes

- Las `criteria_true` por defecto de `agenda`, `pendientes`, `entorno` y `widgets` SHALL declarar el **saludo de apertura del día** («buenos días», «¿qué tal?») como caso afirmativo, porque en este asistente abre el briefing matutino; `recuerdos` y `web` SHALL NOT declararlo.
- El escenario «Un turno conversacional no expone herramientas enrutables», que hoy ilustra el turno vacío con «un saludo sin petición de acción», SHALL cambiar ese ejemplo (p. ej. «gracias, perfecto»): la regla se mantiene, el saludo deja de ser su arquetipo.
- **No** cambia nada más: ni el umbral, ni el catálogo cerrado, ni las herramientas, ni la API, ni la UI, ni el estado del interruptor (`ROUTER_ENABLED=false`). Sin migración.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/skill-router`: se añade el requisito de las criteria por defecto del briefing y se ajusta el ejemplo del turno conversacional.

## Impact

- `src/orchestrator/skills.rs` (cuatro cadenas `criteria_true`) y sus tests.
- La spec `openspec/specs/orchestrator/skill-router/spec.md`.
- **No** toca `settings` (ninguna migración siembra las claves `SKILL_<ID>_CRITERIA_*`: el valor compilado es el default y el respaldo), ni migraciones, ni el enrutador, ni la API ni la UI.
- **Criterio de aceptación**: con las criteria nuevas y el enrutador forzado, el arnés mide el briefing matutino como cubierto y la cobertura alcanza ~100 % **sin bajar ningún umbral**.
