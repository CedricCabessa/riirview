use std::str::FromStr;

use log::LevelFilter;
use log::{debug, info};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use riirview::tui;

// TODO: auto refresh

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(tokio_main())?;
    runtime.shutdown_background();
    Ok(())
}

async fn tokio_main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv()?;
    let logfile = FileAppender::builder()
        .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
            "{d} - {l} - {m}{n}",
        )))
        .build("log/riirview.log")?;

    let level = dotenvy::var("RUST_LOG").unwrap_or("info".to_string());
    let logconfig = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(LevelFilter::from_str(&level)?),
        )?;
    log4rs::init_config(logconfig)?;

    info!("info");
    debug!("debug");

    tui::run().await?;

    Ok(())
}
