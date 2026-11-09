//! Herramienta `oximeal`: gestiona el recetario, el menú semanal y la lista de
//! la compra contra la API HTTP de un OxiMeal autoalojado.
//!
//! Es una herramienta multioperación (molde [`crate::tools::zennotes`]): despacha
//! por un parámetro `operation` con el enum completo y declara un permiso por
//! operación —lecturas sin confirmación, escrituras aditivas con aviso y las que
//! reemplazan o destruyen con aprobación explícita—. La configuración se resuelve
//! con [`crate::services::oximeal::Oximeal::from_settings`] en cada ejecución; las
//! credenciales M2M nunca aparecen en un error.
//!
//! Toda la validación de argumentos ocurre **antes** de resolver la configuración
//! y de tocar la red: un argumento inválido nunca viaja a OxiMeal.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

#[cfg(test)]
use crate::services::oximeal::OximealConfig;
use crate::services::oximeal::{
    Oximeal, OximealError, PlanEntry, RecentRecipe, RecipeSummary, ShoppingItemInput,
    ShoppingListGroup, WeeklyPlanResult,
};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Cota inferior aceptada para `days`.
pub const MIN_DAYS: u32 = 1;

/// Cota superior aceptada para `days`.
pub const MAX_DAYS: u32 = 365;

/// Tipos de comida válidos en una entrada del plan semanal.
const MEAL_TYPES: [&str; 4] = ["breakfast", "lunch", "dinner", "snack"];

/// Roles válidos de una receta **dentro de una comida** del plan semanal.
///
/// No confundir con [`RECIPE_CATEGORIES`]: la categoría de una receta puede ser
/// `dessert`, pero el **rol** de una entrada del plan lo rechaza OxiMeal
/// (`is_valid_role` en su backend acepta solo `main | salad | side`).
const ROLES: [&str; 3] = ["main", "salad", "side"];

/// Categorías de receta aceptadas por `search_recipes`.
const RECIPE_CATEGORIES: [&str; 5] = ["main", "salad", "side", "dessert", "breakfast"];

/// Operaciones que la herramienta admite, en el orden del enum del esquema.
const OPERATIONS: [&str; 6] = [
    "recent_recipes",
    "search_recipes",
    "plan_week",
    "add_shopping_items",
    "clear_purchased",
    "create_recipe",
];

/// Acceso compartido a la configuración de OxiMeal: la base de datos (de donde
/// `from_settings` la resuelve en cada llamada) más un override disponible solo
/// en tests, para apuntar a un servidor `wiremock` sin tocar el entorno ni los
/// ajustes.
struct OximealAccess {
    db: SqlitePool,
    #[cfg(test)]
    config_override: Option<OximealConfig>,
}

impl OximealAccess {
    fn new(db: SqlitePool) -> Self {
        Self {
            db,
            #[cfg(test)]
            config_override: None,
        }
    }

    #[cfg(test)]
    fn with_config(db: SqlitePool, config: OximealConfig) -> Self {
        Self {
            db,
            config_override: Some(config),
        }
    }

    /// Resuelve la configuración en cada llamada y construye el cliente.
    async fn client(&self) -> Result<Oximeal, ToolError> {
        #[cfg(test)]
        if let Some(config) = &self.config_override {
            return Oximeal::new(config.clone()).map_err(oximeal_error);
        }
        Oximeal::from_settings(&self.db)
            .await
            .map_err(oximeal_error)
    }
}

/// Traduce un [`OximealError`] a un [`ToolError`] accionable en español.
///
/// Ningún mensaje incluye credenciales: el detalle crudo ya viene redactado
/// desde el servicio, que borra el secreto antes de construir el error.
fn oximeal_error(error: OximealError) -> ToolError {
    match error {
        OximealError::NotConfigured => ToolError::ExecutionError(
            "OxiMeal no está configurado: pega las credenciales en Ajustes → \
             Integraciones → OxiMeal."
                .to_string(),
        ),
        OximealError::Unauthorized => ToolError::ExecutionError(
            "OxiMeal rechazó las credenciales M2M: revisa el client_id y el \
             client_secret en Ajustes → Integraciones → OxiMeal."
                .to_string(),
        ),
        OximealError::NotFound => {
            ToolError::NotFound("El recurso solicitado no existe en OxiMeal.".to_string())
        }
        OximealError::InvalidInput(detail) => {
            ToolError::ExecutionError(format!("OxiMeal rechazó la petición: {detail}"))
        }
        OximealError::TokenEndpoint(detail) => {
            ToolError::ExecutionError(format!("el IdP de OxiMeal no entregó un token: {detail}"))
        }
        OximealError::Upstream(detail) => {
            ToolError::ExecutionError(format!("OxiMeal no está disponible: {detail}"))
        }
    }
}

/// Extrae un argumento de cadena obligatorio y no en blanco tras `trim`. No toca
/// la red.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ToolError::InvalidArguments(format!("{key} es obligatorio y no puede estar en blanco"))
        })
}

/// Extrae un argumento de cadena opcional, ignorando los vacíos tras `trim`.
fn optional_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Valida un valor contra un enum cerrado. Ausente o en blanco → `Ok(None)`.
fn parse_enum(args: &Value, key: &str, allowed: &[&str]) -> Result<Option<String>, ToolError> {
    match optional_str(args, key) {
        None => Ok(None),
        Some(value) => {
            if allowed.contains(&value.as_str()) {
                Ok(Some(value))
            } else {
                Err(ToolError::InvalidArguments(format!(
                    "{key} inválido: {value}; debe ser uno de {}",
                    allowed.join(", ")
                )))
            }
        }
    }
}

/// Valida el `days` opcional: ausente → `0` (el servicio aplica su 7 por
/// defecto); presente debe ser un entero de `1..=365`. No toca la red.
fn parse_days(args: &Value) -> Result<u32, ToolError> {
    let Some(raw) = args.get("days") else {
        return Ok(0);
    };
    let value = raw.as_u64().ok_or_else(|| {
        ToolError::InvalidArguments(format!(
            "days inválido: {raw}; debe ser un entero entre {MIN_DAYS} y {MAX_DAYS}"
        ))
    })?;
    if !(u64::from(MIN_DAYS)..=u64::from(MAX_DAYS)).contains(&value) {
        return Err(ToolError::InvalidArguments(format!(
            "days inválido: {value}; debe ser un entero entre {MIN_DAYS} y {MAX_DAYS}"
        )));
    }
    Ok(value as u32)
}

/// Valida una fecha `YYYY-MM-DD` (formato exacto del servidor). No toca la red.
fn validate_date(value: &str) -> Result<(), ToolError> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| ToolError::InvalidArguments(format!("date inválida: {value}; usa YYYY-MM-DD")))
}

/// Construye las entradas del plan semanal validando cada campo. `entries` debe
/// ser un array no vacío.
fn parse_plan_entries(args: &Value) -> Result<Vec<PlanEntry>, ToolError> {
    let entries = args
        .get("entries")
        .and_then(|v| v.as_array())
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| {
            ToolError::InvalidArguments("entries debe ser un array con al menos una entrada".into())
        })?;

    let mut out = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let date = required_str(entry, "date")
            .map_err(|_| context(index, "date es obligatoria y no puede estar en blanco"))?;
        validate_date(date)?;
        let meal_type = parse_enum(entry, "meal_type", &MEAL_TYPES)?
            .ok_or_else(|| context(index, "meal_type es obligatoria"))?;
        let recipe_id = required_str(entry, "recipe_id")
            .map_err(|_| context(index, "recipe_id es obligatorio y no puede estar en blanco"))?;
        let role = parse_enum(entry, "role", &ROLES)?;
        let servings = parse_servings(entry)?;

        out.push(PlanEntry {
            date: date.to_string(),
            meal_type,
            recipe_id: recipe_id.to_string(),
            role,
            servings,
        });
    }
    Ok(out)
}

/// Valida el `servings` opcional (`>= 1`). No toca la red.
fn parse_servings(entry: &Value) -> Result<Option<i64>, ToolError> {
    let Some(raw) = entry.get("servings") else {
        return Ok(None);
    };
    let value = raw
        .as_i64()
        .ok_or_else(|| ToolError::InvalidArguments(format!("servings inválido: {raw}")))?;
    if value < 1 {
        return Err(ToolError::InvalidArguments(format!(
            "servings inválido: {value}; debe ser al menos 1"
        )));
    }
    Ok(Some(value))
}

/// Construye los ingredientes de la compra validando cada campo. `items` debe ser
/// un array no vacío.
fn parse_shopping_items(args: &Value) -> Result<Vec<ShoppingItemInput>, ToolError> {
    let items = args
        .get("items")
        .and_then(|v| v.as_array())
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            ToolError::InvalidArguments("items debe ser un array con al menos un ítem".into())
        })?;

    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let name = required_str(item, "name")
            .map_err(|_| context(index, "name es obligatorio y no puede estar en blanco"))?;
        let unit = required_str(item, "unit")
            .map_err(|_| context(index, "unit es obligatorio y no puede estar en blanco"))?;
        let amount = item
            .get("amount")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| context(index, "amount es obligatorio y debe ser un número"))?;
        if amount < 0.0 {
            return Err(context(index, "amount no puede ser negativo"));
        }
        // La categoría de pasillo no tiene enum declarado por el servidor: se
        // acepta cualquier texto y los vacíos se ignoran.
        let category = optional_str(item, "category");

        out.push(ShoppingItemInput {
            name: name.to_string(),
            amount,
            unit: unit.to_string(),
            category,
        });
    }
    Ok(out)
}

/// Completa un mensaje de validación con el índice de la entrada o el ítem.
fn context(index: usize, detail: &str) -> ToolError {
    ToolError::InvalidArguments(format!("en la posición {index}: {detail}"))
}

/// Valida el `RecipeInput` de `create_recipe` y lo devuelve tal cual.
///
/// Exige `name` y `category` no vacíos y un array `ingredients` no vacío cuyos
/// ítems tengan `name` no vacío y, si traen `amount`, no sea negativo. No toca
/// la red.
///
/// El objeto se devuelve **tal cual** (passthrough): los campos opcionales que
/// admite la Agent API de OxiMeal —entre ellos `instructions` y
/// `prep_time_minutes`, pero también `servings`, `diet_type`, `source_url` y
/// `tags`— viajan sin alterar y no se exige ninguno de ellos.
fn parse_recipe(args: &Value) -> Result<Value, ToolError> {
    let recipe = args
        .get("recipe")
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            ToolError::InvalidArguments("recipe es obligatorio y debe ser un objeto".into())
        })?;

    required_str(recipe, "name")?;
    required_str(recipe, "category")?;

    let ingredients = recipe
        .get("ingredients")
        .and_then(|v| v.as_array())
        .filter(|ingredients| !ingredients.is_empty())
        .ok_or_else(|| {
            ToolError::InvalidArguments(
                "ingredients debe ser un array con al menos un ingrediente".into(),
            )
        })?;

    for (index, ingredient) in ingredients.iter().enumerate() {
        required_str(ingredient, "name")
            .map_err(|_| context(index, "el ingrediente necesita un name no vacío"))?;
        if let Some(amount) = ingredient.get("amount").and_then(|v| v.as_f64()) {
            if amount < 0.0 {
                return Err(context(
                    index,
                    "el amount del ingrediente no puede ser negativo",
                ));
            }
        }
    }

    Ok(recipe.clone())
}

/// Presenta una [`RecentRecipe`] como JSON (el DTO solo implementa `Deserialize`).
fn recent_recipe_json(recipe: &RecentRecipe) -> Value {
    json!({
        "id": recipe.id,
        "name": recipe.name,
        "category": recipe.category,
        "last_planned": recipe.last_planned,
    })
}

/// Presenta una [`RecipeSummary`] como JSON.
fn recipe_summary_json(recipe: &RecipeSummary) -> Value {
    json!({
        "id": recipe.id,
        "name": recipe.name,
        "category": recipe.category,
        "diet_type": recipe.diet_type,
        "prep_time_minutes": recipe.prep_time_minutes,
        "servings": recipe.servings,
        "tags": recipe.tags,
    })
}

/// Presenta un grupo de la lista de la compra como JSON.
fn shopping_group_json(group: &ShoppingListGroup) -> Value {
    json!({ "category": group.category, "items": group.items })
}

/// Presenta el resultado del plan semanal como JSON.
fn weekly_plan_json(result: &WeeklyPlanResult) -> Value {
    let shopping_list: Vec<Value> = result
        .shopping_list
        .iter()
        .map(shopping_group_json)
        .collect();
    json!({ "created": result.created, "shopping_list": shopping_list })
}

/// Herramienta multioperación sobre el recetario y los menús de OxiMeal.
pub struct OximealTool {
    access: OximealAccess,
}

impl OximealTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: OximealAccess::new(db),
        }
    }

    /// `recent_recipes` — recetas planificadas en los últimos `days` días.
    async fn recent_recipes(&self, args: Value) -> Result<ToolResult, ToolError> {
        let days = parse_days(&args)?;
        let client = self.access.client().await?;
        let recipes = client.recent_recipes(days).await.map_err(oximeal_error)?;
        let count = recipes.len();
        let recipes: Vec<Value> = recipes.iter().map(recent_recipe_json).collect();
        Ok(ToolResult {
            success: true,
            data: json!({ "count": count, "recipes": recipes }),
            message: Some(format!("{count} recetas recientes")),
        })
    }

    /// `search_recipes` — búsqueda por texto y/o categoría.
    async fn search_recipes(&self, args: Value) -> Result<ToolResult, ToolError> {
        let query = optional_str(&args, "query");
        let category = parse_enum(&args, "category", &RECIPE_CATEGORIES)?;
        let client = self.access.client().await?;
        let recipes = client
            .search_recipes(query.as_deref(), category.as_deref())
            .await
            .map_err(oximeal_error)?;
        let count = recipes.len();
        let recipes: Vec<Value> = recipes.iter().map(recipe_summary_json).collect();
        Ok(ToolResult {
            success: true,
            data: json!({ "count": count, "recipes": recipes }),
            message: Some(format!("{count} recetas")),
        })
    }

    /// `plan_week` — fija el plan semanal y devuelve la lista de la compra.
    async fn plan_week(&self, args: Value) -> Result<ToolResult, ToolError> {
        let entries = parse_plan_entries(&args)?;
        let client = self.access.client().await?;
        let result = client
            .set_weekly_plan(&entries)
            .await
            .map_err(oximeal_error)?;
        Ok(ToolResult {
            success: true,
            data: weekly_plan_json(&result),
            message: Some(format!("{} comidas planificadas", result.created)),
        })
    }

    /// `add_shopping_items` — suma ingredientes a la lista de la compra.
    async fn add_shopping_items(&self, args: Value) -> Result<ToolResult, ToolError> {
        let items = parse_shopping_items(&args)?;
        let client = self.access.client().await?;
        let groups = client
            .add_shopping_items(&items)
            .await
            .map_err(oximeal_error)?;
        let groups: Vec<Value> = groups.iter().map(shopping_group_json).collect();
        Ok(ToolResult {
            success: true,
            data: json!({ "groups": groups }),
            message: Some(format!("{} ítems añadidos a la compra", items.len())),
        })
    }

    /// `clear_purchased` — purga los ítems ya comprados.
    async fn clear_purchased(&self) -> Result<ToolResult, ToolError> {
        let client = self.access.client().await?;
        let removed = client.delete_completed().await.map_err(oximeal_error)?;
        Ok(ToolResult {
            success: true,
            data: json!({ "removed": removed }),
            message: Some(format!("{removed} ítems comprados eliminados")),
        })
    }

    /// `create_recipe` — crea una receta y devuelve la creada.
    async fn create_recipe(&self, args: Value) -> Result<ToolResult, ToolError> {
        let recipe = parse_recipe(&args)?;
        let client = self.access.client().await?;
        let created = client.create_recipe(&recipe).await.map_err(oximeal_error)?;
        let name = created
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(ToolResult {
            success: true,
            data: created,
            message: Some(if name.is_empty() {
                "Receta creada".to_string()
            } else {
                format!("Receta «{name}» creada")
            }),
        })
    }
}

#[async_trait]
impl Tool for OximealTool {
    fn name(&self) -> &'static str {
        "oximeal"
    }

    fn description(&self) -> &'static str {
        "Gestiona el recetario, el menú semanal y la lista de la compra de \
         OxiMeal. Multioperación: lee con `recent_recipes` (recetas planificadas \
         hace poco) y `search_recipes` (por texto y/o categoría); planifica la \
         semana con `plan_week` (devuelve la lista de la compra); gestiona la \
         compra con `add_shopping_items` y `clear_purchased`; y guarda recetas \
         con `create_recipe`."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": OPERATIONS,
                    "description": "Operación a ejecutar contra OxiMeal"
                },
                "days": {
                    "type": "integer",
                    "minimum": MIN_DAYS,
                    "maximum": MAX_DAYS,
                    "description": "Días hacia atrás de recent_recipes (1–365; 7 por defecto)"
                },
                "query": {
                    "type": "string",
                    "description": "Texto a buscar en search_recipes"
                },
                "category": {
                    "type": "string",
                    "enum": RECIPE_CATEGORIES,
                    "description": "Categoría de receta por la que filtrar en search_recipes"
                },
                "entries": {
                    "type": "array",
                    "description": "Entradas del plan semanal (obligatorio en plan_week)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "date": { "type": "string", "description": "Fecha en formato YYYY-MM-DD" },
                            "meal_type": {
                                "type": "string",
                                "enum": MEAL_TYPES
                            },
                            "recipe_id": { "type": "string" },
                            "role": {
                                "type": "string",
                                "enum": ROLES
                            },
                            "servings": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["date", "meal_type", "recipe_id"]
                    }
                },
                "items": {
                    "type": "array",
                    "description": "Ingredientes a añadir a la compra (obligatorio en add_shopping_items)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "amount": { "type": "number", "minimum": 0 },
                            "unit": { "type": "string" },
                            "category": { "type": "string", "description": "Pasillo o categoría del ingrediente (opcional)" }
                        },
                        "required": ["name", "amount", "unit"]
                    }
                },
                "recipe": {
                    "type": "object",
                    "description": "RecipeInput de OxiMeal para create_recipe. Obligatorios: name, category e ingredients (no vacíos).",
                    "properties": {
                        "name": { "type": "string", "description": "Nombre de la receta" },
                        "category": {
                            "type": "string",
                            "enum": RECIPE_CATEGORIES,
                            "description": "Categoría de la receta"
                        },
                        "ingredients": {
                            "type": "array",
                            "description": "Ingredientes de la receta (al menos uno)",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "amount": { "type": "number", "minimum": 0 },
                                    "unit": { "type": "string" },
                                    "category": { "type": "string" }
                                },
                                "required": ["name"]
                            }
                        },
                        "instructions": {
                            "type": "string",
                            "description": "Instrucciones de elaboración: texto libre con la preparación de la receta"
                        },
                        "prep_time_minutes": {
                            "type": "integer",
                            "minimum": 0,
                            "description": "Tiempo de preparación en minutos (opcional)"
                        },
                        "servings": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Número de raciones (opcional)"
                        },
                        "diet_type": {
                            "type": "string",
                            "description": "Tipo de dieta de la receta (opcional)"
                        },
                        "source_url": {
                            "type": "string",
                            "description": "URL de origen de la receta (opcional)"
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Etiquetas de la receta (opcional)"
                        }
                    },
                    "required": ["name", "category", "ingredients"]
                }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|v| v.as_str()) {
            // Lecturas: no modifican nada.
            Some("recent_recipes") | Some("search_recipes") => Permission::NoConfirm,
            // Escrituras aditivas: avisan.
            Some("add_shopping_items") | Some("create_recipe") => Permission::Notify,
            // Reemplazan el plan o destruyen lo comprado: aprobación explícita.
            Some("plan_week") | Some("clear_purchased") => Permission::ExplicitApproval,
            // Operación desconocida o ausente: fail-closed. `execute` la
            // rechazará antes de tocar la red.
            _ => Permission::ExplicitApproval,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        match args.get("operation").and_then(|v| v.as_str()) {
            Some("recent_recipes") => self.recent_recipes(args).await,
            Some("search_recipes") => self.search_recipes(args).await,
            Some("plan_week") => self.plan_week(args).await,
            Some("add_shopping_items") => self.add_shopping_items(args).await,
            Some("clear_purchased") => self.clear_purchased().await,
            Some("create_recipe") => self.create_recipe(args).await,
            Some(other) => Err(ToolError::InvalidArguments(format!(
                "Operación desconocida: {other}"
            ))),
            None => Err(ToolError::InvalidArguments(
                "Falta el argumento obligatorio `operation`".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    /// Config apuntando al servidor `wiremock`; usa el propio servidor como
    /// emisor para montar el descubrimiento OIDC sin red externa.
    fn config_for(server: &MockServer) -> OximealConfig {
        OximealConfig {
            base_url: server.uri(),
            resource: format!("{}/api", server.uri()),
            issuer_url: server.uri(),
            client_id: "client-id".to_string(),
            client_secret: "SECRET".to_string(),
        }
    }

    /// Herramienta atada al servidor `wiremock`.
    async fn tool_for(server: &MockServer) -> OximealTool {
        let db = setup_db().await;
        OximealTool {
            access: OximealAccess::with_config(db, config_for(server)),
        }
    }

    /// Monta el descubrimiento OIDC y el endpoint de token del servidor.
    async fn mount_auth(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "token_endpoint": format!("{}/token", server.uri()),
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "tok-1",
                "expires_in": 3600,
            })))
            .mount(server)
            .await;
    }

    // -----------------------------------------------------------------------
    // Superficie: nombre, esquema y permisos por operación
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_name() {
        let db = setup_db().await;
        assert_eq!(OximealTool::new(db).name(), "oximeal");
    }

    /// Scenario: `parameters()` declara el enum completo y `operation` required.
    #[tokio::test]
    async fn test_parameters_declares_full_operation_enum_and_required() {
        let db = setup_db().await;
        let params = OximealTool::new(db).parameters();

        assert_eq!(params["type"], "object");
        let ops: Vec<String> = params["properties"]["operation"]["enum"]
            .as_array()
            .expect("operation must declare an enum")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        for op in OPERATIONS {
            assert!(
                ops.contains(&op.to_string()),
                "falta la operación `{op}`: {ops:?}"
            );
        }
        assert_eq!(ops.len(), 6, "el enum debe tener seis operaciones: {ops:?}");
        assert_eq!(params["required"], json!(["operation"]));

        let properties = params["properties"].as_object().unwrap();
        for field in ["days", "query", "category", "entries", "items", "recipe"] {
            assert!(
                properties.contains_key(field),
                "falta el argumento `{field}`"
            );
        }
    }

    /// Scenario: El esquema de `create_recipe` documenta las instrucciones.
    ///
    /// El `recipe` de `create_recipe` debe declarar, además de los obligatorios
    /// `name`, `category` e `ingredients`, los campos **opcionales** que la Agent
    /// API de OxiMeal admite en su `RecipeInput`, empezando por `instructions`.
    #[tokio::test]
    async fn test_parameters_document_create_recipe_optional_fields() {
        let db = setup_db().await;
        let params = OximealTool::new(db).parameters();

        let recipe = &params["properties"]["recipe"];
        let props = recipe["properties"]
            .as_object()
            .expect("recipe debe declarar `properties` con los campos de RecipeInput");

        for field in [
            "name",
            "category",
            "ingredients",
            "instructions",
            "prep_time_minutes",
            "servings",
            "diet_type",
            "source_url",
            "tags",
        ] {
            assert!(
                props.contains_key(field),
                "el esquema de `recipe` debe documentar `{field}`: {props:?}"
            );
        }

        assert_eq!(
            recipe["required"],
            json!(["name", "category", "ingredients"]),
            "`name`, `category` e `ingredients` siguen siendo los obligatorios"
        );

        let instructions = recipe["properties"]["instructions"]["description"]
            .as_str()
            .unwrap_or_default();
        assert!(
            instructions.to_lowercase().contains("preparaci"),
            "la descripción de `instructions` debe aludir a la preparación: {instructions:?}"
        );
    }

    /// Scenario: la tabla completa de permisos por operación.
    #[tokio::test]
    async fn test_permission_table() {
        let db = setup_db().await;
        let tool = OximealTool::new(db);

        for op in ["recent_recipes", "search_recipes"] {
            assert_eq!(
                tool.permission(&json!({ "operation": op })),
                Permission::NoConfirm,
                "`{op}` es de solo lectura"
            );
        }
        for op in ["add_shopping_items", "create_recipe"] {
            assert_eq!(
                tool.permission(&json!({ "operation": op })),
                Permission::Notify,
                "`{op}` es una escritura aditiva"
            );
        }
        for op in ["plan_week", "clear_purchased"] {
            assert_eq!(
                tool.permission(&json!({ "operation": op })),
                Permission::ExplicitApproval,
                "`{op}` reemplaza o destruye"
            );
        }
        // Operación desconocida o ausente: fail-closed.
        assert_eq!(
            tool.permission(&json!({ "operation": "empty-recipe-book" })),
            Permission::ExplicitApproval
        );
        assert_eq!(tool.permission(&json!({})), Permission::ExplicitApproval);
    }

    // -----------------------------------------------------------------------
    // Validación de argumentos — sin red
    // -----------------------------------------------------------------------

    /// Scenario: argumentos inválidos → `InvalidArguments` sin tocar la red.
    #[tokio::test]
    async fn test_invalid_arguments_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server).await;

        let cases = [
            json!({ "operation": "empty-recipe-book" }),
            json!({}),
            json!({ "operation": "recent_recipes", "days": 0 }),
            json!({ "operation": "recent_recipes", "days": 366 }),
            json!({ "operation": "search_recipes", "category": "postre" }),
            json!({ "operation": "plan_week", "entries": [] }),
            json!({ "operation": "plan_week", "entries": [{
                "date": "2026/11/09", "meal_type": "dinner", "recipe_id": "a"
            }] }),
            json!({ "operation": "plan_week", "entries": [{
                "date": "2026-11-09", "meal_type": "brunch", "recipe_id": "a"
            }] }),
            json!({ "operation": "plan_week", "entries": [{
                "date": "2026-11-09", "meal_type": "dinner", "recipe_id": "a", "role": "starter"
            }] }),
            // `dessert` es una **categoría** de receta válida, pero **no** un rol
            // de entrada del plan: OxiMeal lo rechaza con 400, así que aquí debe
            // caer sin tocar la red.
            json!({ "operation": "plan_week", "entries": [{
                "date": "2026-11-09", "meal_type": "dinner", "recipe_id": "a", "role": "dessert"
            }] }),
            json!({ "operation": "plan_week", "entries": [{
                "date": "2026-11-09", "meal_type": "dinner", "recipe_id": "a", "servings": 0
            }] }),
            json!({ "operation": "plan_week", "entries": [{
                "date": "2026-11-09", "meal_type": "dinner", "recipe_id": ""
            }] }),
            json!({ "operation": "add_shopping_items", "items": [] }),
            json!({ "operation": "add_shopping_items", "items": [{
                "name": "", "amount": 1, "unit": "ud"
            }] }),
            json!({ "operation": "add_shopping_items", "items": [{
                "name": "Tomate", "amount": -1, "unit": "ud"
            }] }),
            json!({ "operation": "add_shopping_items", "items": [{
                "name": "Tomate", "amount": 1, "unit": ""
            }] }),
            json!({ "operation": "create_recipe", "recipe": { "category": "main", "ingredients": [{"name": "X"}] } }),
            json!({ "operation": "create_recipe", "recipe": { "name": "Tortilla", "category": "main", "ingredients": [] } }),
        ];

        for args in cases {
            let err = tool.execute(args.clone()).await.unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidArguments(_)),
                "se esperaba InvalidArguments para {args}: {err:?}"
            );
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un argumento inválido no debe enviar ninguna petición"
        );
    }

    /// Scenario: La validación acepta instrucciones sin tocar la red.
    ///
    /// `parse_recipe` hace *passthrough* del objeto `recipe`: un `instructions`
    /// no vacío viaja sin alterar y la validación no abre ninguna conexión (por
    /// eso el test no levanta siquiera un servidor `wiremock`).
    #[test]
    fn test_parse_recipe_accepts_instructions_without_network() {
        let args = json!({
            "operation": "create_recipe",
            "recipe": {
                "name": "Tortilla",
                "category": "main",
                "ingredients": [{ "name": "Huevo", "amount": 3, "unit": "ud" }],
                "instructions": "Batir los huevos y cuajar en la sartén.",
                "prep_time_minutes": 15
            }
        });

        let parsed = parse_recipe(&args).expect("un `instructions` no vacío debe aceptarse");
        assert_eq!(
            parsed.get("instructions").and_then(|v| v.as_str()),
            Some("Batir los huevos y cuajar en la sartén.")
        );
        assert_eq!(
            parsed.get("prep_time_minutes").and_then(|v| v.as_i64()),
            Some(15)
        );
    }

    /// Scenario: La validación sigue rechazando lo que OxiMeal rechazaría.
    #[test]
    fn test_parse_recipe_still_rejects_empty_ingredients() {
        let args = json!({
            "operation": "create_recipe",
            "recipe": {
                "name": "Tortilla",
                "category": "main",
                "ingredients": [],
                "instructions": "Batir y cuajar."
            }
        });

        let err = parse_recipe(&args).unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "un `ingredients` vacío debe seguir rechazándose: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Traducción de errores
    // -----------------------------------------------------------------------

    /// Scenario: errores traducidos a mensajes accionables en español, sin
    /// credenciales.
    #[test]
    fn test_error_translation_is_actionable_and_secret_free() {
        let not_configured = oximeal_error(OximealError::NotConfigured);
        assert!(matches!(not_configured, ToolError::ExecutionError(_)));
        let text = not_configured.to_string();
        assert!(text.contains("OxiMeal no está configurado"), "{text}");
        assert!(text.contains("Ajustes"), "{text}");

        let unauthorized = oximeal_error(OximealError::Unauthorized).to_string();
        assert!(
            unauthorized.contains("M2M") || unauthorized.contains("client_id"),
            "{unauthorized}"
        );

        assert!(
            matches!(
                oximeal_error(OximealError::NotFound),
                ToolError::NotFound(_)
            ),
            "404 debe traducirse a NotFound"
        );

        let upstream = oximeal_error(OximealError::Upstream("boom".to_string())).to_string();
        assert!(
            upstream.contains("OxiMeal no está disponible"),
            "{upstream}"
        );

        let token_phase =
            oximeal_error(OximealError::TokenEndpoint("boom".to_string())).to_string();
        assert!(
            token_phase.contains("IdP") && token_phase.contains("token"),
            "un fallo de la fase de token debe decirlo: {token_phase}"
        );

        let invalid =
            oximeal_error(OximealError::InvalidInput("categoría mala".to_string())).to_string();
        assert!(invalid.contains("categoría mala"), "{invalid}");

        for error in [
            OximealError::NotConfigured,
            OximealError::Unauthorized,
            OximealError::NotFound,
            OximealError::InvalidInput("detalle".to_string()),
            OximealError::TokenEndpoint("detalle".to_string()),
            OximealError::Upstream("detalle".to_string()),
        ] {
            assert!(
                !oximeal_error(error).to_string().contains("SECRET"),
                "ningún mensaje debe filtrar credenciales"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Happy path: despacho a la Agent API
    // -----------------------------------------------------------------------

    /// Scenario: `recent_recipes` consulta la Agent API y mapea la respuesta.
    #[tokio::test]
    async fn test_recent_recipes_dispatches_to_the_agent_api() {
        let server = MockServer::start().await;
        mount_auth(&server).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .and(header("authorization", "Bearer tok-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "recipes": [{"id": "a", "name": "A", "category": "main", "last_planned": "2026-11-06"}]
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "recent_recipes", "days": 7 }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["count"], 1);
        assert_eq!(result.data["recipes"][0]["id"], "a");
    }
}
