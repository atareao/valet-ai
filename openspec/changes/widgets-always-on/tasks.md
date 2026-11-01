# Tasks — widgets-always-on

## 1. Planificación (SDD)

- [x] 1.1 Crear el change `openspec/changes/widgets-always-on/`.
- [x] 1.2 Escribir `proposal.md` (why / what changes / impact) con los datos medidos en producción.
- [x] 1.3 Escribir `design.md` (D1–D6, riesgos, plan de migración).
- [x] 1.4 Escribir el delta `specs/orchestrator/skill-router/spec.md`: cinco requisitos modificados
      (el catálogo con el umbral del widget, la selección con umbral cero, la inyección de
      fragmentos, la guía de widgets y la configuración de umbrales). Escribir además el delta
      `specs/db/schema/spec.md`: un requisito modificado (la sección de widgets termina en el
      fragmento, no en el prompt base).
- [x] 1.5 Validar con `openspec validate widgets-always-on --strict`.
- [x] 1.6 **Aprobación explícita del usuario** antes de escribir código o tests.

## 2. RED — tests del umbral cero y de la inyección

- [x] 2.1 `src/orchestrator/skills.rs`: ajustar los **tres** tests del catálogo que el umbral `0`
      invalida —no uno—, porque el escenario «El widget se enruta con umbral propio» dejaba de
      cumplirse y los tres codifican invariantes ligadas a `0.20`:
      - `domain_skills_share_threshold_010_and_widgets_is_000` (:788): pasa a exigir `0` para
        `widgets`, renombrado en consecuencia.
      - `widgets_is_the_only_skill_with_a_zero_threshold` (:815): **es el test que encarna el
        escenario retirado** (`openspec/specs/orchestrator/skill-router/spec.md:41`, «es más alto que
        el de las skills de dominio»). Se sustituye por su espejo: `widgets` es la **única** skill a
        umbral `0` del catálogo, y esa es la condición que la hace incondicional.
      - `conditioned_skills_declare_a_positive_threshold` (:776): acotada a las skills
        **condicionadas**; la excepción es exactamente la lista de skills a umbral `0` (hoy, solo
        `widgets`), de modo que añadir otra obligue a declararla aquí.
- [x] 2.2 `src/orchestrator/skill_router.rs`: test de que una skill a umbral efectivo `0` se
      selecciona con `probability = 0.00`.
- [x] 2.3 `src/orchestrator/skill_router.rs`: test de que la **ausencia** de respuesta **para ella**,
      con respuesta presente para otras skills, no la desactiva (hoy `answers.get(spec.id)` ausente
      la descarta). El test SHALL incluir al menos una respuesta para otra skill: una respuesta
      íntegramente vacía o con ids desconocidos es un **fallo abierto** por el requisito «El enrutador
      SHALL fallar abierto», que este change no modifica, y ahí la selección va vacía a propósito
      —la guía viaja por la regla nueva del fragmento, no por la selección—.
- [x] 2.4 `src/orchestrator/skill_router.rs`: test de que una skill a umbral `0` **deshabilitada** no
      se selecciona, no expone `render_widget` y no inyecta su fragmento.
- [x] 2.5 `src/orchestrator/skill_router.rs`: test de que un umbral mayor que cero sigue
      condicionando (regresión: `0.10` con umbral `0.20` no selecciona).
- [x] 2.6 `src/orchestrator/skill_router.rs`: test de que el **fallo abierto** inyecta el fragmento
      de la skill a umbral cero aunque `selection.skills` vaya vacío, y de que sigue ofreciendo
      `render_widget`.
- [x] 2.7 `src/orchestrator/skill_router.rs`: test de que el override `ROUTER_THRESHOLD_WIDGETS` a
      `0.20` sigue ganando sobre el compilado `0` (la palanca del usuario).
- [x] 2.8 Ejecutar `cargo test` y **confirmar el fallo** de los tests nuevos (el resto en verde).
- [x] 2.9 Los **dos tests heredados** de `effective_threshold` que el umbral `0` deja sin poder
      discriminante: `effective_threshold_falls_back_to_the_compiled_when_override_absent` (ajustando
      el valor del catálogo contra el que compara) y el reapuntado a `agenda` (con `widgets` compilado
      a `0.0`, un `Some(0)` no distingue «override ignorado» de «recortado a 0», así que la prueba se
      muda a una skill de umbral positivo).
- [x] 2.10 **Cascada del set de migraciones**: el estado final ya no materializa
      `ROUTER_THRESHOLD_WIDGETS`, así que pierden su supuesto `test_skill_router_tuning_seeds_defaults`
      y `test_skill_selection_removes_the_global_threshold`, y
      `test_skill_router_tuning_respects_user_values` pasa a escribir el valor del usuario con
      **upsert** (que es la acción real del usuario). `test_skill_router_tuning_is_idempotent` se
      repara **midiendo la idempotencia de verdad** —aplicar dos veces y comparar las dos
      fotografías— y no contra el estado post-migraciones, porque la migración vieja **resucita** la
      clave borrada al reejecutarse. El helper `reconstruct_pre_skill_selection` repone también el
      override histórico que daba por hecho. En `tests/api/tools.rs`,
      `test_list_skills_exposes_effective_values_without_overrides` deja de marcar `threshold` como
      campo sobrescrito de `widgets`. Y en `src/orchestrator/agent.rs`, dos tests del enrutado suman
      `render_widget` al conjunto de herramientas esperado y ponen la precondición que la spec exige
      («ninguna skill a umbral cero») para seguir midiendo el turno sin herramientas enrutables.

## 3. GREEN — el router

- [x] 3.1 `src/orchestrator/skills.rs`: `threshold: 0.0` en el `SkillSpec` de `widgets`, con el
      comentario de por qué (no condicionada por el clasificador).
- [x] 3.2 `src/orchestrator/skill_router.rs`: en `select`, una skill cuyo umbral efectivo sea `0` se
      selecciona aunque `response.answers` no traiga entrada para ella.
- [x] 3.3 `src/orchestrator/skill_router.rs`: en `compose_skill_fragments`, considerar activa una
      skill habilitada con umbral efectivo `0`, con independencia del `SelectionSource`.
- [x] 3.4 Ejecutar `cargo test` y `cargo check` sin errores.

## 4. La migración y la guía

- [x] 4.1 Escribir `migrations/20261101000001_widgets_always_on.sql`: borrar
      `ROUTER_THRESHOLD_WIDGETS` solo si vale `0.20`, y sustituir `SKILL_WIDGETS_PROMPT` solo si su
      valor es byte a byte el sembrado por `20261008000001`.
- [x] 4.2 Transcribir la guía nueva a la migración: el texto exacto, ya redactado y aprobado, está en
      `design.md` → «La guía nueva (texto exacto)». Se copia carácter a carácter; no se reescribe.
- [x] 4.3 Test de la migración: el umbral sembrado se borra y el personalizado se respeta; el
      fragmento sembrado se sustituye y el editado se respeta; la migración es idempotente.
- [x] 4.4 Test del catálogo: el fragmento por defecto nombra `QuickForm`, `Checklist` y
      `LocationWidget`, menciona `render_widget`, enuncia la regla de las opciones, declara esa regla
      prioritaria sobre los casos sin widget y acota esos casos a la explicación conceptual **sin
      opciones ni listas que marcar** y al dato único directo.

## 5. REFACTOR y suite completa

- [x] 5.1 `cargo clippy -- -D warnings` sin avisos.
- [x] 5.2 `cargo fmt --check`.
- [x] 5.3 Revisar que no queda código muerto: los caminos de umbral cero no deben duplicar lógica de
      `effective_threshold`. **Cerrado en 7.2**: la definición de «activa» se unifica en `is_active`.
- [x] 5.4 `just check-all` verde.

## 6. Verificación end-to-end

- [x] 6.1 `openspec validate --all --strict` sin errores.
- [x] 6.2 Comprobar que `render_widget` figura en `exposed_tools` y que el fragmento viaja en el
      mensaje de sistema, también con `ROUTER_ENABLED=false`. **Cubierto por los tres tests
      end-to-end de `agent.rs` que añade 7.3** (enrutado, enrutador apagado y clasificador fallando).
- [ ] 6.3 **Batería medida sobre el despliegue** (es la verificación de la guía, que no tiene test
      unitario). Con el release desplegado, repetir los tres casos reales que fallaron el 1-nov:
      - «que mas tenemos que despachar?» → debía renderizar un widget de opciones (respondió A/B en prosa);
      - «donde utilizo nicegui?» → debía renderizar un widget de opciones (respondió A/B en prosa);
      - «ghostty versión 1.4 no está para Arch?» → debía renderizar un widget de opciones (respondió A/B en prosa).
      Criterio de aceptación: **los tres renderizan un widget**. Si no, la guía no basta y hay que
      abrir la fase determinista (detección de listas de opciones en la respuesta).
- [ ] 6.4 Comprobar en la BD de producción que `messages.widgets` deja de estar vacío y que
      `tools_used` registra `render_widget` en los turnos de la batería.
- [ ] 6.5 `openspec archive widgets-always-on` una vez aprobado el resultado.

## 7. Auditoría y corrección (1-nov-2026)

- [x] 7.1 Auditoría de **solo lectura** del change con `@rust-reviewer`: un fallo crítico, ocho riesgos
      y cinco huecos de cobertura, con fichero y línea.
- [x] 7.2 **Corregir el fallo crítico**: la inyección del fragmento por umbral cero **no llegaba
      nunca** al mensaje de sistema en los caminos de fallo abierto. `read_skill_fragments` filtraba
      por `selection.skills` —que va vacía en esos caminos—, así que el mapa llegaba vacío y la rama
      nueva de `compose_skill_fragments` era **código muerto**: `render_widget` se exponía **sin** su
      guía, que es justo el estado degradado que el change debía tapar. Se unifica la definición de
      «skill activa» en `is_active` / `active_skills` —no deshabilitada **y** (en la selección **o**
      incondicionada)— y el sitio de lectura pasa el conjunto activo en vez de la selección cruda.
      Con eso, herramientas y guía se rigen por la **misma** regla y no pueden divergir.
- [x] 7.3 **Tests end-to-end** (los que faltaban: el test previo construía el mapa de fragmentos a
      mano y nunca pasaba por el lector, así que certificaba un comportamiento inexistente): la guía
      viaja en el mensaje de sistema en un turno enrutado, con el enrutador apagado y con el
      clasificador fallando.
- [x] 7.4 **Cerrar los huecos de cobertura** señalados: una skill deshabilitada no inyecta tampoco en
      los caminos de fallo abierto; y el test del contenido de la guía cubre ya el punto 1 (la «Regla
      de oro»), el punto 6 (el procesamiento de la respuesta) y la **negativa** del punto 4 (la guía
      no puede reintroducir la autorización genérica de prosa de la guía retirada).
- [ ] 7.5 Desplegar y ejecutar la batería de producción (6.3). Es el cierre del change: el arreglo de
      7.2 es lo que hace que la guía llegue de verdad al modelo.
