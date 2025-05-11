pub mod config;
pub mod dirs;
pub mod gh;
pub mod models;
pub mod schema;
pub mod score;
pub mod service;
pub mod tui;

use crate::config::Config;
use diesel::SqliteConnection;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool, PooledConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

type DbConnectionManager = ConnectionManager<SqliteConnection>;
type DbConnection = PooledConnection<DbConnectionManager>;

pub fn get_connection_pool() -> Pool<ConnectionManager<SqliteConnection>> {
    let database_url = Config::get().db_path;
    let manager = ConnectionManager::new(database_url);
    Pool::builder()
        .connection_customizer(Box::new(ConnectionCustomizer {}))
        .build(manager)
        .expect("Could not build connection pool")
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

pub fn run_db_migrations(conn: &mut impl MigrationHarness<diesel::sqlite::Sqlite>) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Could not run migrations");
}

#[derive(Debug)]
struct ConnectionCustomizer {}

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for ConnectionCustomizer {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute("PRAGMA journal_mode = WAL;")
            .map_err(diesel::r2d2::Error::QueryError)
    }
}
