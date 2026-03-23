use std::fs;
use std::path::Path;
use chrono::Local;
use sqlx::{AnyConnection, Connection, Executor};

pub struct Migrator {
    db_url: String,
}

impl Migrator {
    pub fn new(db_url: &str) -> Self {
        Self { db_url: db_url.to_string() }
    }

    pub async fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("migrations");
        if !path.exists() {
            fs::create_dir(path)?;
            println!("Created 'migrations/' directory.");
        }

        let mut conn = AnyConnection::connect(&self.db_url).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS _migrations (id INTEGER PRIMARY KEY, name TEXT, applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)")
            .execute(&mut conn)
            .await?;
        
        println!("Database migration tracking initialized.");
        Ok(())
    }

    pub fn create(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let timestamp = Local::now().format("%Y%m%d%H%M%S");
        let filename = format!("migrations/{}_{}.sql", timestamp, name);
        fs::write(&filename, "-- Migration SQL script\n")?;
        println!("Created migration file: {}", filename);
        Ok(())
    }

    pub async fn up(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = AnyConnection::connect(&self.db_url).await?;
        
        // 1. Get applied migrations
        let applied: Vec<String> = sqlx::query_as::<_, (String,)>("SELECT name FROM _migrations")
            .fetch_all(&mut conn)
            .await?
            .into_iter()
            .map(|r| r.0)
            .collect();

        // 2. Read migration files
        let mut entries: Vec<_> = fs::read_dir("migrations")?
            .filter_map(|e| e.ok())
            .collect();
        
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let file_name = entry.file_name().into_string().unwrap();
            if file_name.ends_with(".sql") && !applied.contains(&file_name) {
                println!("Applying migration: {}", file_name);
                let content = fs::read_to_string(entry.path())?;
                
                // Execute migration
                conn.execute(content.as_str()).await?;
                
                // Record success
                sqlx::query("INSERT INTO _migrations (name) VALUES (?)")
                    .bind(file_name)
                    .execute(&mut conn)
                    .await?;
            }
        }

        println!("Migrations completed.");
        Ok(())
    }

    pub async fn status(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = AnyConnection::connect(&self.db_url).await?;
        let appliedCount: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _migrations")
            .fetch_one(&mut conn)
            .await?;
        
        let totalFiles = fs::read_dir("migrations")?
            .filter(|e| e.as_ref().map(|entry| entry.path().extension().map_or(false, |ex| ex == "sql")).unwrap_or(false))
            .count();
        
        println!("Status: {} applied, {} pending.", appliedCount, totalFiles as i64 - appliedCount);
        Ok(())
    }
}
