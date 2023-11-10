use clap::{Parser, Subcommand};
use libriirview::json::{Category, CategoryDetail, Pr};
use prettytable::{row, Table};
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use tokio;

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Category {
        cat: Option<String>,
        #[clap(long)]
        name: Option<String>,
        #[clap(long)]
        repos: Option<Vec<String>>,
    },
    Sync,
    Prs {
        cat: String,
    },
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Category { cat, name, repos } => match (cat, name, repos) {
            (Some(cat), None, None) => get_category(cat).await?,
            (None, Some(name), _) => create_category(name).await?,
            (Some(cat), name, repos) => update_category(cat, name, repos).await?,
            (None, _, _) => get_categories().await?,
        },
        Commands::Sync => sync().await?,
        Commands::Prs { cat } => get_prs(cat).await?,
    }
    Ok(())
}

async fn get_categories() -> Result<(), Box<dyn Error>> {
    let res = reqwest::get("http://localhost:8000/categories");
    let cats = res.await?.json::<Vec<Category>>().await?;

    let mut table = Table::new();
    table.add_row(row!["uid", "name"]);
    cats.iter().for_each(|cat| {
        table.add_row(row![cat.uid, cat.name]);
    });

    table.printstd();

    Ok(())
}

async fn get_category(uid: String) -> Result<(), Box<dyn Error>> {
    let res = reqwest::get(format!("http://localhost:8000/categories/{}", uid));
    let cat = res.await?.json::<CategoryDetail>().await?;

    let mut table = Table::new();
    table.add_row(row!["uid", "name"]);
    table.add_row(row![cat.category.uid, cat.category.name,]);

    table.printstd();

    let mut table = Table::new();
    table.add_row(row!["repo"]);
    cat.repos.iter().for_each(|r| {
        table.add_row(row![r]);
    });

    table.printstd();

    Ok(())
}

#[derive(Serialize)]
struct UpdateCategory {
    name: Option<String>,
    repos: Option<Vec<String>>,
}

async fn update_category(
    cat: String,
    name: Option<String>,
    repos: Option<Vec<String>>,
) -> Result<(), Box<dyn Error>> {
    let body = UpdateCategory { name, repos };
    let client = reqwest::Client::new();
    client
        .post(format!("http://localhost:8000/categories/{}", cat))
        .json(&body)
        .send()
        .await?;

    Ok(())
}

async fn create_category(name: String) -> Result<(), Box<dyn Error>> {
    let mut body = HashMap::new();
    body.insert("name", name);
    let client = reqwest::Client::new();
    client
        .post("http://localhost:8000/categories")
        .json(&body)
        .send()
        .await?;

    Ok(())
}

async fn get_prs(cat: String) -> Result<(), Box<dyn Error>> {
    let res = reqwest::get(format!("http://localhost:8000/categories/{}/prs", cat));
    let cat = res.await?.json::<HashMap<String, Vec<Pr>>>().await?;

    for (repo, prs) in cat {
        println!("{}", repo);
        let mut table = Table::new();
        table.add_row(row!["title", "url", "date"]);
        for pr in prs {
            table.add_row(row![pr.title, pr.url, pr.updated_at]);
        }
        table.printstd();
    }

    Ok(())
}

async fn sync() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    client
        .post("http://localhost:8000/sync".to_string())
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
