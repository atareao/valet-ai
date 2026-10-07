//! Catálogo cerrado de skills y su relación con las herramientas.
//!
//! Una *skill* es una habilidad del asistente que agrupa las herramientas de
//! un dominio **incluyendo sus prerrequisitos** (p. ej. `clima` cubre
//! `weather` y `geocode`). El conjunto *core* (`render_widget`,
//! `get_current_time`) no se enruta y se expone siempre.
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
    Tareas,
    Recordatorios,
    Notas,
    Clima,
    Lugares,
    BusquedaWeb,
    Memoria,
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
    /// Herramientas que cubre, **incluyendo prerrequisitos**.
    pub tools: &'static [&'static str],
    /// Clave del fragmento de prompt (p. ej. `"SKILL_AGENDA_PROMPT"`).
    pub prompt_key: &'static str,
    /// Encabezado de la sección del fragmento.
    pub prompt_heading: &'static str,
}

/// Herramientas siempre expuestas y **nunca** enrutables.
pub const CORE_TOOLS: &[&str] = &["render_widget", "get_current_time"];

/// Catálogo cerrado de skills, en el orden canónico que sigue el resto del
/// módulo (orden del catálogo).
static CATALOG: [SkillSpec; 8] = [
    SkillSpec {
        skill: Skill::Agenda,
        id: "agenda",
        instructions: "¿Se necesita consultar o modificar la agenda: eventos, citas, reuniones, cumpleaños o disponibilidad?",
        criteria_true: "El usuario pregunta por eventos, citas, reuniones o cumpleaños, o pide crearlos, moverlos o cancelarlos, o pregunta por su disponibilidad o huecos libres",
        criteria_false: "El usuario no menciona eventos, citas, reuniones ni cumpleaños, no pide crearlos, moverlos ni cancelarlos, y no pregunta por su disponibilidad ni por huecos libres",
        tools: &["calendar"],
        prompt_key: "SKILL_AGENDA_PROMPT",
        prompt_heading: "# SKILL ACTIVA: AGENDA",
    },
    SkillSpec {
        skill: Skill::Tareas,
        id: "tareas",
        instructions: "¿Se necesita gestionar tareas: listarlas, crearlas, actualizarlas o completarlas?",
        criteria_true: "El usuario habla de tareas, pendientes, cosas por hacer, prioridades o el estado de algo que debe completar",
        criteria_false: "El usuario no habla de tareas, pendientes, cosas por hacer ni prioridades, y no pregunta por el estado de algo que deba completar",
        tools: &["tasks"],
        prompt_key: "SKILL_TAREAS_PROMPT",
        prompt_heading: "# SKILL ACTIVA: TAREAS",
    },
    SkillSpec {
        skill: Skill::Recordatorios,
        id: "recordatorios",
        instructions: "¿Se necesita poner, listar, posponer o descartar un recordatorio o una alarma?",
        criteria_true: "El usuario pide que le avisen o le recuerden algo, o habla de recordatorios, alarmas o avisos temporales",
        criteria_false: "El usuario no pide que le avisen ni le recuerden algo, y no habla de recordatorios, alarmas ni avisos temporales",
        tools: &["reminders"],
        prompt_key: "SKILL_RECORDATORIOS_PROMPT",
        prompt_heading: "# SKILL ACTIVA: RECORDATORIOS",
    },
    SkillSpec {
        skill: Skill::Notas,
        id: "notas",
        instructions: "¿Se necesita gestionar notas personales?",
        criteria_true: "El usuario pide guardar, recuperar, listar o clasificar una nota, una idea, un journal o un dato suelto que quiere conservar",
        criteria_false: "El usuario no pide guardar, recuperar, listar ni clasificar una nota, una idea, un journal ni un dato suelto que quiera conservar",
        tools: &["notes"],
        prompt_key: "SKILL_NOTAS_PROMPT",
        prompt_heading: "# SKILL ACTIVA: NOTAS",
    },
    SkillSpec {
        skill: Skill::Clima,
        id: "clima",
        instructions: "¿Se necesita consultar el clima actual o una previsión meteorológica?",
        criteria_true: "El usuario pregunta por el tiempo, la temperatura, la lluvia o la previsión, en un lugar concreto o donde está",
        criteria_false: "El usuario no pregunta por el tiempo, la temperatura, la lluvia ni la previsión, ni en un lugar concreto ni donde está",
        tools: &["weather", "geocode", "get_current_location"],
        prompt_key: "SKILL_CLIMA_PROMPT",
        prompt_heading: "# SKILL ACTIVA: CLIMA",
    },
    SkillSpec {
        skill: Skill::Lugares,
        id: "lugares",
        instructions: "¿Se necesita buscar un lugar, establecimiento o servicio, o resolver una dirección o unas coordenadas?",
        criteria_true: "El usuario busca dónde está o dónde hay algo (negocios, servicios, puntos de interés), o pide convertir una dirección en coordenadas o unas coordenadas en una dirección",
        criteria_false: "El usuario no busca dónde está ni dónde hay algo (negocios, servicios, puntos de interés), y no pide convertir una dirección en coordenadas ni unas coordenadas en una dirección",
        tools: &["search_places", "geocode", "reverse_geocode", "get_current_location"],
        prompt_key: "SKILL_LUGARES_PROMPT",
        prompt_heading: "# SKILL ACTIVA: LUGARES",
    },
    SkillSpec {
        skill: Skill::BusquedaWeb,
        id: "busqueda_web",
        instructions: "¿Se necesita buscar información en la web?",
        criteria_true: "El usuario pide información externa o actual: noticias, documentación, precios, o hechos que hay que comprobar fuera de sus propios datos",
        criteria_false: "El usuario no pide información externa ni actual (noticias, documentación, precios), ni hechos que haya que comprobar fuera de sus propios datos",
        tools: &["web_search"],
        prompt_key: "SKILL_BUSQUEDA_WEB_PROMPT",
        prompt_heading: "# SKILL ACTIVA: BÚSQUEDA WEB",
    },
    SkillSpec {
        skill: Skill::Memoria,
        id: "memoria",
        instructions: "¿Se necesita buscar en el historial: conversaciones anteriores, notas, eventos o tareas del propio usuario?",
        criteria_true: "El usuario pregunta por algo ya hablado, por lo que dijo o le dijiste antes, o pide buscar de forma transversal en sus mensajes, notas, eventos o tareas",
        criteria_false: "El usuario no pregunta por algo ya hablado ni por lo que dijo o le dijiste antes, y no pide buscar de forma transversal en sus mensajes, notas, eventos ni tareas",
        tools: &["unified_search"],
        prompt_key: "SKILL_MEMORIA_PROMPT",
        prompt_heading: "# SKILL ACTIVA: MEMORIA",
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

    /// Las ocho variantes del enum, para exigir que el catálogo las cubra
    /// todas exactamente una vez.
    const ALL_SKILLS: [Skill; 8] = [
        Skill::Agenda,
        Skill::Tareas,
        Skill::Recordatorios,
        Skill::Notas,
        Skill::Clima,
        Skill::Lugares,
        Skill::BusquedaWeb,
        Skill::Memoria,
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
    fn clima_and_lugares_cover_their_prerequisites() {
        let clima = catalog().iter().find(|spec| spec.skill == Skill::Clima);
        assert!(clima.is_some(), "clima must be in the catalog");
        let clima = clima.unwrap();
        assert!(
            clima.tools.contains(&"weather"),
            "clima must cover weather: {:?}",
            clima.tools
        );
        assert!(
            clima.tools.contains(&"geocode"),
            "clima must cover its prerequisite geocode: {:?}",
            clima.tools
        );

        let lugares = catalog().iter().find(|spec| spec.skill == Skill::Lugares);
        assert!(lugares.is_some(), "lugares must be in the catalog");
        let lugares = lugares.unwrap();
        for tool in ["search_places", "geocode", "reverse_geocode"] {
            assert!(
                lugares.tools.contains(&tool),
                "lugares must cover {tool}: {:?}",
                lugares.tools
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
        let selected = [Skill::Agenda, Skill::Clima, Skill::Lugares, Skill::Tareas];
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
        assert_eq!(skill_of_tool("weather"), Some(Skill::Clima));
        assert_eq!(skill_of_tool("render_widget"), None);
        assert_eq!(skill_of_tool("get_current_time"), None);
        assert_eq!(skill_of_tool("no_existe"), None);
    }
}
