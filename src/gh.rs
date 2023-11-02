use log::info;
use regex::Regex;
use reqwest::Response;
use rocket::futures::{self, StreamExt, TryStreamExt};
use serde::Deserialize;
use std::env;

#[derive(Deserialize, Debug)]
pub struct Notification {
    unread: bool,
    updated_at: String, // chrono
    subject: Subject,
    repository: Repository,
}

#[derive(Deserialize, Debug)]
pub struct Subject {
    title: String,
    url: String,
    r#type: String,
}

#[derive(Deserialize, Debug)]
pub struct Repository {
    full_name: String,
}

impl Notification {
    pub fn title(&self) -> &String {
        &self.subject.title
    }

    pub fn url(&self) -> String {
        let api_url = &self.subject.url; // https://api.github.com/repos/LedgerHQ/<repo>/pulls/N
        let num = api_url.split("/").last().unwrap();

        format!(
            "https://github.com/{}/pull/{}",
            self.repository.full_name, num
        )
    }

    pub fn repo(&self) -> &String {
        &self.repository.full_name
    }

    pub fn r#type(&self) -> &String {
        &self.subject.r#type
    }

    pub fn updated_at(&self) -> &String {
        &self.updated_at
    }

    pub fn unread(&self) -> bool {
        self.unread
    }
}

fn pages_from_link(link: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let regex = Regex::new(r#"<(.*)\?page=(\d+)>; rel="next", <.*\?page=(\d+)>; rel="last""#);

    let matches = regex?.captures(link);
    if let Some(matches) = matches {
        let url = matches.get(1).unwrap().as_str();
        let second: u32 = matches.get(2).unwrap().as_str().parse()?;
        let last: u32 = matches.get(3).unwrap().as_str().parse()?;

        let mut urls = vec![];
        for page in second..last {
            urls.push(format!("{}?page={}", url, page))
        }
        Ok(urls)
    } else {
        Ok(vec![])
    }
}

async fn get_url(url: String) -> Result<Response, Box<dyn std::error::Error>> {
    info!("url {}", url);
    let client = reqwest::Client::new();
    let token = env::var("GH_TOKEN")?;
    let resp = client
        .get(url)
        .header("User-Agent", "reqwest")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;
    Ok(resp)
}

async fn get_notifications(url: String) -> Result<Vec<Notification>, Box<dyn std::error::Error>> {
    let resp = get_url(url.into()).await?;
    Ok(resp.json::<Vec<Notification>>().await?)
}

pub async fn gh() -> Result<Vec<Notification>, Box<dyn std::error::Error>> {
    let resp = get_url("https://api.github.com/notifications".into()).await?;

    let mut repos = if let Some(link) = resp.headers().get("link") {
        let link = link.to_str()?;
        let urls = pages_from_link(link)?;

        futures::stream::iter(urls)
            .map(|url| get_notifications(url))
            .buffer_unordered(30)
            .try_fold(vec![], |mut acc, x| async {
                acc.extend(x);
                Ok(acc)
            })
            .await?
    } else {
        vec![]
    };

    let res = resp.json::<Vec<Notification>>().await?;
    repos.extend(res);

    Ok(repos)
}
