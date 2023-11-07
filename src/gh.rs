use log::{debug, info};
use regex::Regex;
use reqwest::Response;
use rocket::futures::{self, StreamExt, TryStreamExt};
use serde::Deserialize;
use std::env;
use url::Url;

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

fn url_to_page(url: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let url = Url::parse(url)?;
    let page = url
        .query_pairs()
        .find(|(key, _)| key == "page")
        .ok_or("no page")?
        .1
        .into_owned()
        .parse()?;

    Ok(page)
}

fn pages_from_link(link: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let regex = Regex::new(r#"<(.*)>; rel="next", <(.*)>; rel="last""#);

    let matches = regex?.captures(link);
    if let Some(matches) = matches {
        let next = matches.get(1).ok_or("link parse error")?.as_str();
        let last = matches.get(2).ok_or("link parse error")?.as_str();
        // choose "next" as base url, then change "page" query param
        let url = Url::parse(next)?;

        let next = url_to_page(next)?;
        let last = url_to_page(last)?;
        debug!("last page {}", last);
        let mut urls = vec![];
        for page in next..last + 1 {
            let mut new_url = url.clone();
            let query = url.query_pairs().filter(|(name, _)| name != "page");

            new_url
                .query_pairs_mut()
                .clear()
                .extend_pairs(query)
                .extend_pairs([("page", format!("{}", page))]);

            debug!("add url {}", new_url.to_string());

            urls.push(new_url.to_string());
        }
        Ok(urls)
    } else {
        Err("invalid link format".into())
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
    //TODO: add since
    let resp = get_url("https://api.github.com/notifications?all=true".into()).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_basic() {
        let link = r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=4>; rel="last""#;

        assert_eq!(
            pages_from_link(link).unwrap(),
            vec![
                "https://api.github.com/notifications?page=2",
                "https://api.github.com/notifications?page=3",
                "https://api.github.com/notifications?page=4"
            ]
        );
    }
    #[test]
    fn test_link_nonext() {
        let link = r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=2>; rel="last""#;

        assert_eq!(
            pages_from_link(link).unwrap(),
            vec!["https://api.github.com/notifications?page=2",]
        );
    }

    #[test]
    fn test_link_invalid() {
        let link = "not a link";

        assert!(pages_from_link(link).is_err());
    }

    #[test]
    fn test_link() {
        let link = r#"<https://api.github.com/notifications?all=true&since=2023-11-06T00%3A00%3A00Z&page=2>; rel="next", <https://api.github.com/notifications?all=true&since=2023-11-06T00%3A00%3A00Z&page=4>; rel="last""#;

        assert_eq!(
            pages_from_link(link).unwrap(),
            vec![
		"https://api.github.com/notifications?all=true&since=2023-11-06T00%3A00%3A00Z&page=2",
		"https://api.github.com/notifications?all=true&since=2023-11-06T00%3A00%3A00Z&page=3",
		"https://api.github.com/notifications?all=true&since=2023-11-06T00%3A00%3A00Z&page=4"
            ]
        );
    }
}
