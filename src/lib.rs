pub mod dirs;
pub mod gh;
pub mod models;
pub mod schema;
pub mod score;
pub mod service;
pub mod tui;

use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use log::debug;
use std::env;

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let directories = dirs::Directories::new();
    let database_url = match env::var("DATABASE_URL") {
        Ok(val) => val,
        Err(_) => {
            let db_path = directories.data.join("riirview.db");
            db_path.to_str().unwrap().into()
        }
    };
    debug!("Connecting to {}", database_url);
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

pub fn run_db_migrations(conn: &mut impl MigrationHarness<diesel::sqlite::Sqlite>) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Could not run migrations");
}
