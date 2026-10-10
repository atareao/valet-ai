//! Catálogo cerrado de skills y su relación con las herramientas.
//!
//! Una *skill* es una habilidad del asistente que agrupa las herramientas de
//! un dominio **incluyendo sus prerrequisitos** (p. ej. `entorno` cubre
//! `weather` y `geocode`). El conjunto *core* (`get_current_time`,
//! `get_current_location`) no se enruta y se expone siempre.
//!
//! Este módulo es la fuente de verdad del catálogo; el router y el arnés de
//! evaluación lo consumen. Una herramienta registrada debe pertenecer al core
//! o a alguna skill (se verifica en los tests de integridad), de modo que
//! añadir una tool y olvidarla en el catálogo haga fallar los tests en vez de
//! degradar el enrutado.

/// Habilidad enrutable del asistente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Skill {
    Agenda,
    Pendientes,
    Recuerdos,
    Entorno,
    Web,
    Widgets,
    Running,
}

/// Especificación cerrada de una skill.
pub struct SkillSpec {
    /// Variante de la skill.
    pub skill: Skill,
    /// Id estable (p. ej. `"agenda"`), usado en logs y en `settings`.
    pub id: &'static str,
    /// Pregunta tipada para Jev.
    pub instructions: &'static str,
    /// Criteria del «sí» (primitiva `noul`).
    pub criteria_true: &'static str,
    /// Criteria del «no» (primitiva `noul`).
    pub criteria_false: &'static str,
    /// Umbral por defecto de la skill (se usa si no hay override en settings).
    pub threshold: f32,
    /// Herramientas que cubre, **incluyendo prerrequisitos**.
    pub tools: &'static [&'static str],
    /// Clave del fragmento de prompt (p. ej. `"SKILL_AGENDA_PROMPT"`).
    pub prompt_key: &'static str,
    /// **Marcador de duplicado**: coincide con la primera línea del fragmento
    /// (su encabezado real). [`compose_skill_fragments`] omite el fragmento si
    /// el prompt base ya contiene esta cadena, de modo que la guía no viaja dos
    /// veces.
    ///
    /// [`compose_skill_fragments`]: crate::orchestrator::skill_router::compose_skill_fragments
    pub prompt_heading: &'static str,
}

/// Herramientas siempre expuestas y **nunca** enrutables.
pub const CORE_TOOLS: &[&str] = &["get_current_time", "get_current_location"];

/// Catálogo cerrado de skills, en el orden canónico que sigue el resto del
/// módulo (orden del catálogo).
static CATALOG: [SkillSpec; 7] = [
    SkillSpec {
        skill: Skill::Agenda,
        id: "agenda",
        instructions: "¿La respuesta requiere mirar o cambiar la agenda (eventos, citas, reuniones, cumpleaños, disponibilidad)?",
        criteria_true: "El mensaje se refiere a eventos, citas, reuniones, cumpleaños, calendario, disponibilidad o huecos libres, o a qué tiene el usuario en un momento o un día. Cuenta también un saludo de apertura del día («buenos días», «¿qué tal?»), que en este asistente abre un resumen del día.",
        criteria_false: "El mensaje no se refiere a nada programado en el tiempo ni pide planificar nada en una fecha.",
        threshold: 0.10,
        tools: &["calendar"],
        prompt_key: "SKILL_AGENDA_PROMPT",
        prompt_heading: "# SKILL ACTIVA: AGENDA",
    },
    SkillSpec {
        skill: Skill::Pendientes,
        id: "pendientes",
        instructions: "¿La respuesta requiere gestionar tareas por hacer, recordatorios o alarmas?",
        criteria_true: "El mensaje se refiere a tareas, pendientes, cosas por hacer, prioridades, recordatorios, alarmas o avisos a una hora. Cuenta también un saludo de apertura del día, que abre un repaso de lo que hay pendiente.",
        criteria_false: "El mensaje no se refiere a pendientes ni a ningún aviso.",
        threshold: 0.10,
        tools: &["tasks", "reminders"],
        prompt_key: "SKILL_PENDIENTES_PROMPT",
        prompt_heading: "# SKILL ACTIVA: PENDIENTES",
    },
    SkillSpec {
        skill: Skill::Recuerdos,
        id: "recuerdos",
        instructions: "¿La respuesta requiere guardar o recuperar notas, apuntes o algo ya hablado?",
        criteria_true: "El mensaje pide apuntar, guardar, recuperar o listar un texto, una idea o un diario, o pregunta por algo ya hablado.",
        criteria_false: "El mensaje no pide guardar ni recuperar información personal del usuario ni busca en el historial.",
        threshold: 0.10,
        tools: &["notes", "unified_search"],
        prompt_key: "SKILL_RECUERDOS_PROMPT",
        prompt_heading: "# SKILL ACTIVA: RECUERDOS",
    },
    SkillSpec {
        skill: Skill::Entorno,
        id: "entorno",
        instructions: "¿La respuesta requiere el tiempo, un lugar, una dirección o unas coordenadas?",
        criteria_true: "El mensaje pregunta por el tiempo o la previsión, busca dónde hay algo o dónde está algo, o pide resolver una dirección o unas coordenadas. Cuenta también un saludo de apertura del día, que abre la previsión del tiempo.",
        criteria_false: "El mensaje no pregunta por el tiempo ni por lugares, direcciones o coordenadas.",
        threshold: 0.10,
        tools: &["weather", "geocode", "reverse_geocode", "search_places"],
        prompt_key: "SKILL_ENTORNO_PROMPT",
        prompt_heading: "# SKILL ACTIVA: ENTORNO",
    },
    SkillSpec {
        skill: Skill::Web,
        id: "web",
        instructions: "¿La respuesta requiere información externa de internet?",
        criteria_true: "El mensaje pide información que no está en los datos del usuario: noticias, documentación, precios, datos de una empresa, un producto o una persona, o cualquier hecho que haya que comprobar.",
        criteria_false: "El mensaje se responde con datos del propio usuario o de su entorno, o no necesita internet.",
        threshold: 0.10,
        tools: &["web_search"],
        prompt_key: "SKILL_WEB_PROMPT",
        prompt_heading: "# SKILL ACTIVA: WEB",
    },
    SkillSpec {
        skill: Skill::Widgets,
        id: "widgets",
        instructions: "¿La respuesta requiere mostrar algo interactivo en pantalla (formulario, lista para marcar, opciones, mapa o datos geográficos)?",
        criteria_true: "El turno implica pedir varios datos a la vez, dar una lista para marcar, ofrecer una elección entre opciones, presentar un plan con pasos, o mostrar direcciones, un mapa, una tabla de datos geográficos o estadísticas. Cuenta también si el usuario pide expresamente un widget, un formulario, un checklist, un plano o un mapa. Cuenta también un saludo de apertura del día, que abre un panel con los datos del día.",
        criteria_false: "El turno se resuelve con una explicación, un dato o una lista de texto.",
        threshold: 0.20,
        tools: &["render_widget"],
        prompt_key: "SKILL_WIDGETS_PROMPT",
        // The real heading of the seeded fragment (copied verbatim from the
        // system prompt by the migration), so the anti-duplication rule detects
        // it even when the user edited the block and it was not removed.
        prompt_heading: "# Instrucciones de Interfaz y Widgets Interactivos",
    },
    SkillSpec {
        skill: Skill::Running,
        id: "running",
        instructions: "¿La respuesta requiere mirar las sesiones de running del usuario (qué ha corrido, cómo fue una sesión, su ritmo o su frecuencia cardíaca, o sus totales)?",
        criteria_true: "El mensaje se refiere a las carreras o sesiones de running del usuario: qué ha corrido o cuándo, cómo fue una sesión concreta, su ritmo (min/km), su frecuencia cardíaca, su cadencia o su desnivel, o sus totales y estadísticas de atleta.",
        criteria_false: "El mensaje no pregunta por las sesiones de running del usuario ni por sus estadísticas de atleta.",
        threshold: 0.10,
        tools: &[
            "strava_recent_activities",
            "strava_activity_detail",
            "strava_activity_streams",
            "strava_athlete_stats",
        ],
        prompt_key: "SKILL_RUNNING_PROMPT",
        prompt_heading: "# SKILL ACTIVA: RUNNING",
    },
];

/// Catálogo cerrado de skills, en el orden canónico del sistema.
pub fn catalog() -> &'static [SkillSpec] {
    &CATALOG
}

/// Unión de las herramientas de las skills dadas, **sin duplicados** y en
/// orden determinista (orden del catálogo). No incluye el conjunto core.
pub fn tools_for(skills: &[Skill]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for spec in catalog() {
        if !skills.contains(&spec.skill) {
            continue;
        }
        for tool in spec.tools {
            let name = *tool;
            if !out.iter().any(|t| t.as_str() == name) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Skill enrutable a la que pertenece `tool`, si alguna. `None` para el core
/// o para nombres desconocidos.
pub fn skill_of_tool(tool: &str) -> Option<Skill> {
    if CORE_TOOLS.contains(&tool) {
        return None;
    }
    catalog()
        .iter()
        .find(|spec| spec.tools.contains(&tool))
        .map(|spec| spec.skill)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las siete variantes del enum, para exigir que el catálogo las cubra
    /// todas exactamente una vez.
    const ALL_SKILLS: [Skill; 7] = [
        Skill::Agenda,
        Skill::Pendientes,
        Skill::Recuerdos,
        Skill::Entorno,
        Skill::Web,
        Skill::Widgets,
        Skill::Running,
    ];

    /// Nombres de las herramientas del registry de producción.
    async fn registry_tool_names() -> Vec<String> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");

        let registry = crate::build_tool_registry(&pool);
        let mut names: Vec<String> = registry
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn every_registered_tool_is_covered_by_core_or_a_skill() {
        let names = registry_tool_names().await;

        let mut orphans: Vec<String> = Vec::new();
        for name in &names {
            let in_core = CORE_TOOLS.contains(&name.as_str());
            let in_skill = catalog()
                .iter()
                .any(|spec| spec.tools.contains(&name.as_str()));
            if !in_core && !in_skill {
                orphans.push(name.clone());
            }
        }

        assert!(
            orphans.is_empty(),
            "tools not covered by CORE_TOOLS or any skill (routing would silently degrade): {orphans:?}"
        );
    }

    #[test]
    fn every_skill_variant_is_in_the_catalog_exactly_once() {
        let cat = catalog();

        for skill in ALL_SKILLS {
            let count = cat.iter().filter(|spec| spec.skill == skill).count();
            assert_eq!(
                count, 1,
                "skill {skill:?} must appear exactly once in the catalog"
            );
        }

        let mut ids: Vec<&str> = cat.iter().map(|spec| spec.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(count, ids.len(), "skill ids must be unique: {ids:?}");
    }

    #[test]
    fn every_skill_declares_non_empty_content() {
        let cat = catalog();
        assert!(!cat.is_empty(), "the closed catalog must not be empty");

        for spec in cat {
            assert!(
                !spec.id.trim().is_empty(),
                "skill {:?} must declare an id",
                spec.skill
            );
            assert!(
                !spec.instructions.trim().is_empty(),
                "skill {:?} must declare instructions",
                spec.skill
            );
            assert!(
                !spec.criteria_true.trim().is_empty(),
                "skill {:?} must declare criteria_true",
                spec.skill
            );
            assert!(
                !spec.criteria_false.trim().is_empty(),
                "skill {:?} must declare criteria_false",
                spec.skill
            );
            assert!(
                !spec.prompt_key.trim().is_empty(),
                "skill {:?} must declare prompt_key",
                spec.skill
            );
            assert!(
                !spec.prompt_heading.trim().is_empty(),
                "skill {:?} must declare prompt_heading",
                spec.skill
            );
        }
    }

    #[test]
    fn entorno_covers_its_prerequisites() {
        let entorno = catalog().iter().find(|spec| spec.skill == Skill::Entorno);
        assert!(entorno.is_some(), "entorno must be in the catalog");
        let entorno = entorno.unwrap();
        for tool in ["weather", "geocode", "reverse_geocode", "search_places"] {
            assert!(
                entorno.tools.contains(&tool),
                "entorno must cover {tool}: {:?}",
                entorno.tools
            );
        }
    }

    #[test]
    fn core_tools_are_not_routable() {
        let cat = catalog();
        assert!(!cat.is_empty(), "the closed catalog must not be empty");

        for spec in cat {
            for core in CORE_TOOLS {
                assert!(
                    !spec.tools.contains(core),
                    "skill {:?} must not route core tool {core}",
                    spec.skill
                );
            }
        }
    }

    #[test]
    fn tools_for_has_no_duplicates_and_follows_catalog_order() {
        let selected = [Skill::Agenda, Skill::Entorno, Skill::Pendientes];
        let tools = tools_for(&selected);

        assert!(
            !tools.is_empty(),
            "tools_for must resolve the tools of the selected skills"
        );

        let mut sorted = tools.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            tools.len(),
            sorted.len(),
            "tools_for must not return duplicates: {tools:?}"
        );

        let expected: Vec<String> = catalog()
            .iter()
            .filter(|spec| selected.contains(&spec.skill))
            .flat_map(|spec| spec.tools.iter().map(|t| t.to_string()))
            .fold(Vec::new(), |mut acc, tool| {
                if !acc.contains(&tool) {
                    acc.push(tool);
                }
                acc
            });
        assert_eq!(
            tools, expected,
            "tools_for must follow the catalog order with dedup"
        );
    }

    #[test]
    fn skill_of_tool_maps_catalog_tools_and_not_core() {
        assert_eq!(skill_of_tool("calendar"), Some(Skill::Agenda));
        assert_eq!(skill_of_tool("weather"), Some(Skill::Entorno));
        assert_eq!(skill_of_tool("render_widget"), Some(Skill::Widgets));
        assert_eq!(
            skill_of_tool("strava_recent_activities"),
            Some(Skill::Running)
        );
        assert_eq!(skill_of_tool("get_current_time"), None);
        assert_eq!(skill_of_tool("get_current_location"), None);
        assert_eq!(skill_of_tool("no_existe"), None);
    }

    // ─── RED: catálogo de seis dominios amplios (skill-router-tuning 1.1) ────
    //
    // El contrato es el diseño medido: seis ids canónicos, la agrupación por
    // dominio, el core reducido y los seis fragmentos. Estos tests describen ese
    // contrato y **fallan** con el catálogo actual (ocho skills finas).

    /// Herramientas de una skill por su id, o vacío si el id no existe todavía
    /// (para que el test falle por aserción y no por pánico).
    fn tools_of(id: &str) -> Vec<&'static str> {
        catalog()
            .iter()
            .find(|spec| spec.id == id)
            .map(|spec| spec.tools.to_vec())
            .unwrap_or_default()
    }

    /// Umbral por defecto de una skill por su id, o `None` si aún no existe.
    fn threshold_of(id: &str) -> Option<f32> {
        catalog()
            .iter()
            .find(|spec| spec.id == id)
            .map(|spec| spec.threshold)
    }

    /// Porcentaje entero del umbral, para comparar sin ruido de coma flotante.
    fn pct(t: f32) -> i32 {
        (t * 100.0).round() as i32
    }

    #[test]
    fn catalog_declares_exactly_the_seven_wide_domain_ids() {
        let ids: Vec<&str> = catalog().iter().map(|spec| spec.id).collect();

        for id in [
            "agenda",
            "pendientes",
            "recuerdos",
            "entorno",
            "web",
            "widgets",
            "running",
        ] {
            assert!(
                ids.contains(&id),
                "el catálogo debe declarar la skill `{id}`; tiene {ids:?}"
            );
        }
        assert_eq!(
            ids.len(),
            7,
            "el catálogo debe tener exactamente siete skills; tiene {ids:?}"
        );
    }

    #[test]
    fn no_legacy_skill_ids_remain() {
        let ids: Vec<&str> = catalog().iter().map(|spec| spec.id).collect();

        for legacy in [
            "tareas",
            "recordatorios",
            "notas",
            "clima",
            "lugares",
            "busqueda_web",
            "memoria",
        ] {
            assert!(
                !ids.contains(&legacy),
                "el id antiguo `{legacy}` no debe permanecer; tiene {ids:?}"
            );
        }
    }

    #[test]
    fn grouping_places_each_domain_tool_in_one_skill() {
        let pendientes = tools_of("pendientes");
        for tool in ["tasks", "reminders"] {
            assert!(
                pendientes.contains(&tool),
                "pendientes debe cubrir {tool}: {pendientes:?}"
            );
        }

        let recuerdos = tools_of("recuerdos");
        for tool in ["notes", "unified_search"] {
            assert!(
                recuerdos.contains(&tool),
                "recuerdos debe cubrir {tool}: {recuerdos:?}"
            );
        }

        let entorno = tools_of("entorno");
        for tool in ["weather", "geocode", "reverse_geocode", "search_places"] {
            assert!(
                entorno.contains(&tool),
                "entorno debe cubrir {tool}: {entorno:?}"
            );
        }

        assert!(
            tools_of("agenda").contains(&"calendar"),
            "agenda debe cubrir calendar: {:?}",
            tools_of("agenda")
        );
        assert!(
            tools_of("web").contains(&"web_search"),
            "web debe cubrir web_search: {:?}",
            tools_of("web")
        );
        assert!(
            tools_of("widgets").contains(&"render_widget"),
            "widgets debe cubrir render_widget: {:?}",
            tools_of("widgets")
        );

        let running = tools_of("running");
        for tool in [
            "strava_recent_activities",
            "strava_activity_detail",
            "strava_activity_streams",
            "strava_athlete_stats",
        ] {
            assert!(
                running.contains(&tool),
                "running debe cubrir {tool}: {running:?}"
            );
        }
    }

    #[test]
    fn core_is_time_and_location_and_widget_is_not_core() {
        assert_eq!(
            CORE_TOOLS,
            &["get_current_time", "get_current_location"],
            "el core debe ser get_current_time y get_current_location"
        );
        assert!(
            !CORE_TOOLS.contains(&"render_widget"),
            "render_widget no debe pertenecer al core"
        );
        for spec in catalog() {
            assert!(
                !spec.tools.contains(&"get_current_location"),
                "get_current_location es core: no debe estar en la skill {}",
                spec.id
            );
        }
        assert!(
            tools_of("widgets").contains(&"render_widget"),
            "render_widget debe enrutarse con la skill widgets: {:?}",
            tools_of("widgets")
        );
    }

    #[test]
    fn catalog_plus_core_covers_all_seventeen_tools_without_orphans() {
        let mut covered: Vec<String> = CORE_TOOLS.iter().map(|s| s.to_string()).collect();
        for spec in catalog() {
            for tool in spec.tools {
                covered.push((*tool).to_string());
            }
        }
        covered.sort_unstable();
        covered.dedup();

        assert_eq!(
            covered.len(),
            17,
            "el core más las siete skills deben cubrir las diecisiete herramientas: {covered:?}"
        );
        for spec in catalog() {
            assert!(
                !spec.tools.contains(&"get_current_location"),
                "get_current_location solo puede venir del core, no de {}",
                spec.id
            );
        }
    }

    #[test]
    fn prompt_keys_are_the_seven_canonical_keys() {
        let keys: Vec<&str> = catalog().iter().map(|spec| spec.prompt_key).collect();

        for key in [
            "SKILL_AGENDA_PROMPT",
            "SKILL_PENDIENTES_PROMPT",
            "SKILL_RECUERDOS_PROMPT",
            "SKILL_ENTORNO_PROMPT",
            "SKILL_WEB_PROMPT",
            "SKILL_WIDGETS_PROMPT",
            "SKILL_RUNNING_PROMPT",
        ] {
            assert!(
                keys.contains(&key),
                "falta la clave de fragmento {key}; tiene {keys:?}"
            );
        }
        assert_eq!(
            keys.len(),
            7,
            "el catálogo debe declarar exactamente siete claves de fragmento; tiene {keys:?}"
        );
        for legacy in [
            "SKILL_TAREAS_PROMPT",
            "SKILL_RECORDATORIOS_PROMPT",
            "SKILL_NOTAS_PROMPT",
            "SKILL_CLIMA_PROMPT",
            "SKILL_LUGARES_PROMPT",
            "SKILL_BUSQUEDA_WEB_PROMPT",
            "SKILL_MEMORIA_PROMPT",
        ] {
            assert!(
                !keys.contains(&legacy),
                "la clave antigua {legacy} no debe permanecer; tiene {keys:?}"
            );
        }
    }

    // ─── RED: umbrales por defecto por skill (skill-router-tuning 1.2) ───────

    #[test]
    fn every_skill_declares_a_positive_threshold() {
        for spec in catalog() {
            assert!(
                spec.threshold > 0.0,
                "la skill {} debe declarar un umbral > 0.0 (threshold={})",
                spec.id,
                spec.threshold
            );
        }
    }

    #[test]
    fn domain_skills_share_threshold_010_and_widgets_is_020() {
        for id in [
            "agenda",
            "pendientes",
            "recuerdos",
            "entorno",
            "web",
            "running",
        ] {
            let t = threshold_of(id).unwrap_or(-1.0);
            assert_eq!(
                pct(t),
                10,
                "el dominio `{id}` debe declarar un umbral de 0.10"
            );
        }
        assert_eq!(
            threshold_of("widgets").map(pct),
            Some(20),
            "widgets debe declarar un umbral de 0.20"
        );
    }

    #[test]
    fn widgets_threshold_is_strictly_above_every_domain() {
        let widgets = threshold_of("widgets").unwrap_or(0.0);
        for id in [
            "agenda",
            "pendientes",
            "recuerdos",
            "entorno",
            "web",
            "running",
        ] {
            let domain = threshold_of(id).unwrap_or(0.0);
            assert!(
                widgets > domain,
                "widgets ({widgets}) debe superar el umbral de `{id}` ({domain})"
            );
        }
    }

    // ─── RED: saludo de apertura del día (morning-briefing-criteria) ─────────
    //
    // El «briefing» matinal se reconoce por un marcador textual exacto en la
    // criteria del «sí»: solo las skills del briefing (`agenda`, `pendientes`,
    // `entorno` y `widgets`) deben declararlo, y `recuerdos` y `web` NO. Estas
    // pruebas describen ese contrato y **fallan** con el catálogo actual, en el
    // que ninguna skill menciona el saludo de apertura del día.

    /// Marcador textual exacto que identifica el saludo de apertura del día.
    const MORNING_OPENING_GREETING: &str = "saludo de apertura del día";

    /// `criteria_true` de una skill por su id, o `None` si el id no existe
    /// todavía (para que el test falle por aserción y no por pánico).
    fn criteria_true_of(id: &str) -> Option<&'static str> {
        catalog()
            .iter()
            .find(|spec| spec.id == id)
            .map(|spec| spec.criteria_true)
    }

    #[test]
    fn briefing_skills_declare_the_morning_opening_greeting() {
        for id in ["agenda", "pendientes", "entorno", "widgets"] {
            let criteria = criteria_true_of(id).unwrap_or("");
            assert!(
                criteria.contains(MORNING_OPENING_GREETING),
                "la skill `{id}` debe declarar el saludo de apertura del día en criteria_true; tiene: {criteria:?}"
            );
        }
    }

    #[test]
    fn only_the_briefing_skills_declare_the_morning_opening_greeting() {
        let declaring: Vec<&str> = catalog()
            .iter()
            .filter(|spec| spec.criteria_true.contains(MORNING_OPENING_GREETING))
            .map(|spec| spec.id)
            .collect();

        assert_eq!(
            declaring,
            vec!["agenda", "pendientes", "entorno", "widgets"],
            "solo las skills del briefing deben declarar el saludo de apertura del día; declaran: {declaring:?}"
        );
    }
}
