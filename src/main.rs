use std::str::FromStr;

use anyhow::{Context, Result};
use log::LevelFilter;
use log::{debug, info};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use riirview::{dirs, get_connection_pool, run_db_migrations, tui};

fn main() -> Result<()> {
    if std::env::args().nth(1) == Some("--version".into()) {
        println!("riirview {}", env!("VERGEN_GIT_DESCRIBE"));
    } else {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(tokio_main())?;
        runtime.shutdown_background();
    }
    Ok(())
}

async fn tokio_main() -> Result<()> {
    _ = dotenvy::dotenv();

    let directories = dirs::Directories::new();
    directories.create().context("Error creating directories")?;

    let logfile = FileAppender::builder()
        .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
            "{d} - {l} - {m}{n}",
        )))
        .build(directories.cache.join("riirview.log"))?;

    let level = dotenvy::var("RUST_LOG").unwrap_or("info".to_string());
    let logconfig = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(LevelFilter::from_str(&level)?),
        )?;
    log4rs::init_config(logconfig)?;

    info!(
        "cache {}",
        directories.cache.to_str().unwrap_or("no cache dir")
    );
    info!(
        "config {}",
        directories.config.to_str().unwrap_or("no config dir")
    );
    info!(
        "data {}",
        directories.cache.to_str().unwrap_or("no data dir")
    );
    debug!("debug enabled");

    let mut connection = get_connection_pool().get().unwrap();
    run_db_migrations(&mut connection);

    tui::run().await
}
