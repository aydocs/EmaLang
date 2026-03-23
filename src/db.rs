use sqlx::AnyPool;
use std::fs;
use std::path::Path;
use sqlx::Row;

pub async fn apply_migrations(pool: &AnyPool, dir: &str) -> Result<usize, String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ema_migrations (name TEXT PRIMARY KEY, applied_at TEXT DEFAULT CURRENT_TIMESTAMP)"
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let path = Path::new(dir);
    if !path.exists() {
        return Ok(0);
    }

    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().ok().map(|t| t.is_file()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut applied = 0usize;
    for e in entries {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.to_ascii_lowercase().ends_with(".sql") {
            continue;
        }

        let already: Result<Option<sqlx::any::AnyRow>, _> = sqlx::query(
            "SELECT name FROM ema_migrations WHERE name = $1 LIMIT 1"
        )
        .bind(&name)
        .fetch_optional(pool)
        .await;

        if let Ok(Some(_)) = already {
            continue;
        }

        let sql = fs::read_to_string(e.path()).map_err(|er| er.to_string())?;
        sqlx::query(&sql).execute(pool).await.map_err(|er| format!("Migration {} failed: {}", name, er))?;
        sqlx::query("INSERT INTO ema_migrations (name) VALUES ($1)")
            .bind(&name)
            .execute(pool)
            .await
            .map_err(|er| er.to_string())?;
        applied += 1;
    }

    Ok(applied)
}

