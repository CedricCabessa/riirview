use anyhow::anyhow;
use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use core::fmt;
use futures::stream::iter;
use futures::StreamExt;
use futures::TryStreamExt;
use log::{debug, info};
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::Response;
use reqwest::StatusCode;
use serde::Deserialize;
use std::future::Future;
use url::Url;

#[derive(Deserialize, Debug)]
pub struct Notification {
    pub id: String,
    pub unread: bool,
    pub reason: String,
    #[serde(with = "my_date_format")]
    pub updated_at: NaiveDateTime,
    pub subject: Subject,
    pub repository: Repository,
}

mod my_date_format {
    use chrono::NaiveDateTime;
    use serde::{self, Deserialize, Deserializer};

    const FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)?;
        Ok(dt)
    }
}

#[derive(Deserialize, Debug, PartialEq)]
pub enum NotificationType {
    PullRequest,
    Release,
    Issue,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct Subject {
    pub title: String,
    pub url: Option<String>,
    pub r#type: NotificationType,
}

#[derive(Deserialize, Debug)]
pub struct Repository {
    pub full_name: String,
}

#[derive(Deserialize, Debug)]
pub struct PullRequest {
    pub url: String,
    pub html_url: String,
    pub state: String,
    pub number: i32,
    pub draft: bool,
    pub merged: bool,
    pub user: User,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub login: String,
}

#[derive(Deserialize, Debug)]
pub struct Release {
    pub url: String,
    pub html_url: String,
    pub author: User,
}

#[derive(Deserialize, Debug)]
pub struct Issue {
    pub url: String,
    pub html_url: String,
    pub user: User,
    pub state: String,
}

#[derive(Debug)]
pub struct UpdateStatus {
    pub need_update: bool,
    pub last_update: NaiveDateTime,
    pub poll_interval: u64,
    pub ratelimit_remaining: u64,
    pub ratelimit_used: u64,
}

struct Client {
    base_url: String,
    headers: HeaderMap,
    client: reqwest::Client,
}

impl Client {
    pub fn new() -> Result<Client, Error> {
        let client = reqwest::Client::new();
        let token = dotenvy::var("GH_TOKEN").map_err(|_| Error::MissingToken)?;
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", "riirview".parse().unwrap());
        headers.insert("Accept", "application/vnd.github+json".parse().unwrap());
        headers.insert(
            "Authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );

        Ok(Client {
            base_url: "https://api.github.com".into(),
            headers,
            client,
        })
    }

    pub async fn get_notifications(&self, last_update: Option<NaiveDateTime>) -> Result<Response> {
        let url = match last_update {
            Some(last_update) => {
                let since = last_update
                    .and_local_timezone(Utc)
                    .single()
                    .unwrap()
                    .to_rfc3339()
                    .replace("+00:00", "Z"); //FIXME: we should avoid this stupid replace
                format!("{}/notifications?all=true&since={}", self.base_url, since)
            }
            None => format!("{}/notifications", self.base_url),
        };

        let resp = self.get(url).await?;
        Ok(resp)
    }

    pub async fn mark_notification_done(&self, id: &String) -> Result<()> {
        let url = format!("{}/notifications/threads/{}", self.base_url, id);
        self.del(url).await?;
        Ok(())
    }

    pub async fn mark_notification_read(&self, id: &String) -> Result<()> {
        let url = format!("{}/notifications/threads/{}", self.base_url, id);
        self.patch(url).await?;
        Ok(())
    }

    pub async fn need_update(&self, last_update: NaiveDateTime) -> Result<Response> {
        let url = format!("{}/notifications", self.base_url);
        let mut custom_headers = HeaderMap::new();
        let since = last_update
            .and_local_timezone(Utc)
            .single()
            .unwrap()
            .to_rfc2822();
        custom_headers.insert("If-Modified-Since", since.parse()?);
        self.head(url, Some(custom_headers)).await
    }

    pub async fn get(&self, url: String) -> Result<Response> {
        info!("GET {}", &url);
        let resp = self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .send()
            .await?;
        debug!("status {} for {}", resp.status(), &url);
        Ok(resp.error_for_status()?)
    }

    async fn del(&self, url: String) -> Result<Response> {
        info!("DEL {}", &url);
        let resp = self
            .client
            .delete(&url)
            .headers(self.headers.clone())
            .send()
            .await?;
        debug!("status {} for {}", resp.status(), &url);
        Ok(resp.error_for_status()?)
    }

    async fn patch(&self, url: String) -> Result<Response> {
        info!("PATCH {}", &url);
        let resp = self
            .client
            .patch(&url)
            .headers(self.headers.clone())
            .send()
            .await?;
        debug!("status {} for {}", resp.status(), &url);
        Ok(resp.error_for_status()?)
    }

    async fn head(&self, url: String, headers: Option<HeaderMap>) -> Result<Response> {
        info!("HEAD {} {:?}", &url, headers);
        let builder = self.client.head(&url).headers(self.headers.clone());

        let builder = if let Some(custom_header) = headers {
            builder.headers(custom_header)
        } else {
            builder
        };
        let resp = builder.send().await?;

        debug!("status {} for {}", resp.status(), &url);

        Ok(resp.error_for_status()?)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Error {
    MissingToken,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::MissingToken => write!(f, "Env variable GH_TOKEN is missing"),
        }
    }
}

impl std::error::Error for Error {}

fn url_to_page(url: &str) -> Result<u32> {
    let url = Url::parse(url)?;
    let page = url
        .query_pairs()
        .find(|(key, _)| key == "page")
        .ok_or(anyhow!("no page"))?
        .1
        .into_owned()
        .parse()?;

    Ok(page)
}

fn pages_from_link(link: &str) -> Result<Vec<String>> {
    let regex = Regex::new(r#"<(.*)>; rel="next", <(.*)>; rel="last""#);

    let matches = regex?.captures(link);
    if let Some(matches) = matches {
        let next = matches.get(1).ok_or(anyhow!("link parse error"))?.as_str();
        let last = matches.get(2).ok_or(anyhow!("link parse error"))?.as_str();
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
        Err(anyhow!("invalid link format"))
    }
}

async fn get_notifications(url: String) -> Result<Vec<Notification>> {
    let client = Client::new()?;
    let resp = client.get(url).await?;
    Ok(resp.json::<Vec<Notification>>().await?)
}

async fn get_pr(url: String) -> Result<PullRequest> {
    let client = Client::new()?;
    let resp = client.get(url).await?;
    Ok(resp.json::<PullRequest>().await?)
}

async fn get_release(url: String) -> Result<Release> {
    let client = Client::new()?;
    let resp = client.get(url).await?;
    Ok(resp.json::<Release>().await?)
}

async fn get_issue(url: String) -> Result<Issue> {
    let client = Client::new()?;
    let resp = client.get(url).await?;
    Ok(resp.json::<Issue>().await?)
}

const NB_TASK: usize = 30;

pub async fn fetch_notifications(last_update: Option<NaiveDateTime>) -> Result<Vec<Notification>> {
    let client = Client::new()?;
    let resp = client.get_notifications(last_update).await?;

    let mut notifications = if let Some(link) = resp.headers().get("link") {
        let link = link.to_str()?;
        let urls = pages_from_link(link)?;

        iter(urls)
            .map(get_notifications)
            .buffer_unordered(NB_TASK)
            .try_fold(vec![], |mut acc, x| async {
                acc.extend(x);
                Ok(acc)
            })
            .await?
    } else {
        vec![]
    };

    let res = resp.json::<Vec<Notification>>().await?;
    notifications.extend(res);

    Ok(notifications)
}

pub async fn fetch_prs(notifications: &[Notification]) -> Result<Vec<PullRequest>> {
    fetch_object(notifications, NotificationType::PullRequest, get_pr).await
}

pub async fn fetch_releases(notifications: &[Notification]) -> Result<Vec<Release>> {
    fetch_object(notifications, NotificationType::Release, get_release).await
}

pub async fn fetch_issues(notifications: &[Notification]) -> Result<Vec<Issue>> {
    fetch_object(notifications, NotificationType::Issue, get_issue).await
}

async fn fetch_object<F, Fut, T>(
    notifications: &[Notification],
    notification_type: NotificationType,
    getter: F,
) -> Result<Vec<T>>
where
    Fut: Future<Output = Result<T>>,
    F: Fn(String) -> Fut,
{
    let urls: Vec<String> = notifications
        .iter()
        .filter_map(|notif| {
            if notif.subject.r#type == notification_type {
                notif.subject.url.clone()
            } else {
                None
            }
        })
        .collect();

    iter(urls)
        .map(getter)
        .buffer_unordered(NB_TASK)
        .try_fold(vec![], |mut acc, x| async {
            acc.push(x);
            Ok(acc)
        })
        .await
}

pub async fn mark_as_done(id: &String) -> Result<()> {
    let client = Client::new()?;
    client.mark_notification_done(id).await
}

pub async fn mark_as_done_multiple(ids: &Vec<String>) -> Result<()> {
    iter(ids)
        .map(mark_as_done)
        .buffer_unordered(NB_TASK)
        .try_collect()
        .await
}

pub async fn mark_as_read(id: &String) -> Result<()> {
    let client = Client::new()?;
    client.mark_notification_read(id).await
}

pub async fn check_update_and_limit(last_update: NaiveDateTime) -> Result<UpdateStatus> {
    let client = Client::new()?;
    let resp = client.need_update(last_update).await?;
    let headers = resp.headers();
    let poll_interval = headers
        .get("x-poll-interval")
        .ok_or(anyhow!("no x-poll-interval header"))?
        .to_str()?
        .parse()?;
    let ratelimit_remaining = headers
        .get("x-ratelimit-remaining")
        .ok_or(anyhow!("no x-ratelimit-remaining header"))?
        .to_str()?
        .parse()?;
    let ratelimit_used = headers
        .get("x-ratelimit-used")
        .ok_or(anyhow!("no x-ratelimit-used header"))?
        .to_str()?
        .parse()?;

    Ok(UpdateStatus {
        need_update: resp.status() != StatusCode::NOT_MODIFIED,
        last_update,
        poll_interval,
        ratelimit_remaining,
        ratelimit_used,
    })
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

    #[test]
    fn test_parser() -> Result<()> {
        use chrono::naive::{NaiveDate, NaiveTime};
        use serde_json;
        use std::fs::File;
        use std::io::prelude::*;

        let mut file = File::open("tests/notifications.json")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let notifications: Vec<Notification> = serde_json::from_str(&contents)?;
        assert_eq!(notifications.len(), 1);

        let d = NaiveDate::from_ymd_opt(2025, 1, 19).unwrap();
        let t = NaiveTime::from_hms_opt(8, 43, 54).unwrap();

        let expected = NaiveDateTime::new(d, t);

        assert_eq!(notifications.get(0).unwrap().updated_at, expected);

        Ok(())
    }
}
