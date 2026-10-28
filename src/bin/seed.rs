use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = std::env::var("DATABASE_URL").unwrap_or_else(|_| "valet.db".into());
    let pool = valet::db::init_db(&db_path).await?;

    seed_profiles(&pool).await?;
    seed_events(&pool).await?;
    seed_notes(&pool).await?;

    println!("✅ Seed data created in {}", db_path);
    Ok(())
}

async fn seed_profiles(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();

    // Ana
    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, avatar_url, preferences, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind("profile-ana")
    .bind("Ana")
    .bind(Option::<String>::None)
    .bind("{}")
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;

    // Luis
    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, avatar_url, preferences, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind("profile-luis")
    .bind("Luis")
    .bind(Option::<String>::None)
    .bind("{}")
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;

    println!("  👤 Perfiles: Ana, Luis");
    Ok(())
}

async fn seed_events(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let tomorrow = (Utc::now() + chrono::Duration::days(1)).to_rfc3339();
    let week_later = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    sqlx::query(
        "INSERT OR IGNORE INTO events (id, profile_id, title, description, start_time, end_time, location, scope)
         VALUES (?1, 'profile-ana', 'Cena con amigos', 'Restaurante italiano', ?2, ?3, 'Trattoria Da Mario', 'shared')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .bind(&tomorrow)
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO events (id, profile_id, title, description, start_time, end_time, location, scope)
         VALUES (?1, 'profile-ana', 'Revisión semanal', 'Revisar objetivos y tareas', ?2, ?3, 'Oficina', 'personal')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&tomorrow)
    .bind(&week_later)
    .execute(db)
    .await?;

    println!("  📅 Eventos: Cena con amigos, Revisión semanal");
    Ok(())
}

async fn seed_notes(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR IGNORE INTO notes (id, profile_id, content, category, tags, created_at)
         VALUES (?1, 'profile-ana', 'Idea para app: gestor de recetas con IA', 'idea', 'app,recetas', ?2)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .execute(db)
    .await?;

    let yesterday = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO notes (id, profile_id, content, category, tags, created_at)
         VALUES (?1, 'profile-ana', 'Hoy aprendí sobre sistemas de recomendación', 'journal', 'aprendizaje,IA', ?2)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&yesterday)
    .execute(db)
    .await?;

    println!("  📝 Notas: Idea app, Journal");
    Ok(())
}
