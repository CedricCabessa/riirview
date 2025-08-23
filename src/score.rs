use crate::models::Notification;
use core::fmt;
use log::{debug, error, info};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(PartialEq, Eq, Debug, Clone)]
enum RuleType {
    Author,
    Repo,
    Title,
    Org,
    Reason,
}

#[derive(Deserialize, Debug)]
struct TomlRule {
    rule: String,
    param: String,
    score: i32,
}

#[derive(Debug, Clone)]
pub struct Rule {
    rule: RuleType,
    pub name: String,
    params: Vec<String>,
    pub score: i32,
}

impl Rule {
    pub fn matcher(&self, notification: &Notification) -> i32 {
        let fct = match self.rule {
            RuleType::Author => rule_author,
            RuleType::Repo => rule_repo,
            RuleType::Title => rule_title,
            RuleType::Org => rule_org,
            RuleType::Reason => rule_reason,
        };
        if fct(notification, &self.params) {
            info!(
                "{} match {} score:{}",
                notification.title, self.name, self.score
            );
            self.score
        } else {
            0
        }
    }
}

pub struct Scorer {
    rules: Vec<Rule>,
}

impl Scorer {
    pub fn new(toml_path: PathBuf) -> Result<Scorer, Error> {
        let config_res = fs::read_to_string(toml_path);
        if let Err(ref error) = config_res
            && let Error::RuleFileNotFound = error.into()
        {
            error!("No rules file found!");
            return Ok(Scorer { rules: vec![] });
        }
        let config = config_res?;

        let toml_rules: HashMap<String, TomlRule> = toml::from_str(&config)?;
        let rules: Result<Vec<Rule>, String> = toml_rules
            .iter()
            .map(|(name, r)| {
                Ok::<Rule, String>(Rule {
                    rule: rule_from_str(&r.rule)?,
                    params: r.param.split(",").map(|s| s.trim().into()).collect(),
                    score: r.score,
                    name: name.into(),
                })
            })
            .collect::<Vec<Result<Rule, String>>>()
            .into_iter()
            .collect();
        debug!("rules: {:?}", rules);

        Ok(Scorer { rules: rules? })
    }

    pub fn score(&self, notification: &Notification) -> i32 {
        self.rules
            .iter()
            .fold(0, |acc, rule| acc + rule.matcher(notification))
    }

    pub fn explain(&self, notification: &Notification) -> Vec<Rule> {
        self.rules
            .iter()
            .filter(|rule| rule.matcher(notification) != 0)
            .cloned()
            .collect()
    }
}

fn rule_from_str(rule_name: &str) -> Result<RuleType, String> {
    match rule_name {
        "author" => Ok(RuleType::Author),
        "repo" => Ok(RuleType::Repo),
        "title" => Ok(RuleType::Title),
        "org" => Ok(RuleType::Org),
        "reason" => Ok(RuleType::Reason),
        _ => Err(rule_name.into()),
    }
}

fn rule_author(notification: &Notification, params: &[String]) -> bool {
    params.contains(&notification.author)
}

fn rule_repo(notification: &Notification, params: &[String]) -> bool {
    params.contains(&notification.repo)
}

fn rule_title(notification: &Notification, params: &[String]) -> bool {
    params.iter().any(|p| notification.title.contains(p))
}

fn rule_org(notification: &Notification, params: &[String]) -> bool {
    params.iter().any(|param| {
        let neg = param.starts_with("!");
        let param = if neg {
            if let Some(param) = param.get(1..) {
                param
            } else {
                return false;
            }
        } else {
            param
        };
        (notification.org() == *param) != neg
    })
}

fn rule_reason(notification: &Notification, params: &[String]) -> bool {
    params.iter().any(|p| notification.reason.contains(p))
}

#[derive(Debug)]
pub enum Error {
    RuleFileNotFound,
    InvalidToml,
    InvalidRule(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::RuleFileNotFound => write!(f, "Rule file not found"),
            Error::InvalidToml => write!(f, "Invalid toml file"),
            Error::InvalidRule(msg) => write!(f, "Rule found: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<&io::Error> for Error {
    fn from(_: &io::Error) -> Self {
        Error::RuleFileNotFound
    }
}
impl From<io::Error> for Error {
    fn from(_: io::Error) -> Self {
        Error::RuleFileNotFound
    }
}
impl From<toml::de::Error> for Error {
    fn from(_: toml::de::Error) -> Self {
        Error::InvalidToml
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::InvalidRule(err)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    use crate::models::NotificationState;
    use crate::models::NotificationType;

    use super::*;
    use std::collections::HashSet;

    fn create_notification() -> Notification {
        Notification {
            id: "1".to_string(),
            reason: "participating".to_string(),
            title: "title".into(),
            url: "http://exemple.com".into(),
            type_: NotificationType::PullRequest,
            repo: "torvalds/linux".into(),
            unread: true,
            updated_at: NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
            done: false,
            score: 0,
            state: NotificationState::Open,
            author: "JohnDoe".into(),
            score_boost: 0,
        }
    }

    #[test]
    fn test_scorer_builder() {
        let path = "tests/rules.toml";
        let scorer = Scorer::new(path.into()).unwrap();

        assert_eq!(scorer.rules.len(), 3);
        let display_names: HashSet<String> = scorer.rules.iter().map(|r| r.name.clone()).collect();
        assert_eq!(
            display_names,
            HashSet::from(["me".into(), "friends".into(), "my_fav_repos".into(),])
        );

        let tl_rule = scorer
            .rules
            .iter()
            .find(|r| r.name == "my_fav_repos")
            .unwrap();

        assert_eq!(tl_rule.rule, RuleType::Repo);
        assert_eq!(tl_rule.params, vec!["torvalds/linux", "emacs-mirror/emacs"]);
        assert_eq!(tl_rule.score, 5);
    }

    #[test]
    fn test_scorer_score() {
        let path = "tests/rules.toml";
        let scorer = Scorer::new(path.into()).unwrap();

        let db_notification = create_notification();

        assert_eq!(scorer.score(&db_notification), 105);
    }

    #[test]
    fn test_scorer_title() {
        let notification = create_notification();

        assert_eq!(
            rule_title(
                &notification,
                &vec!["bad title".into(), "title".into(), "another title".into()]
            ),
            true
        );
        assert_eq!(
            rule_title(
                &notification,
                &vec!["bad title".into(), "another title".into()]
            ),
            false
        );
    }

    #[test]
    fn test_scorer_org() {
        let notification = create_notification();

        assert_eq!(rule_org(&notification, &vec!["torvalds".into()]), true);
        assert_eq!(rule_org(&notification, &vec!["!torvalds".into()]), false);
        assert_eq!(rule_org(&notification, &vec!["!rms".into()]), true);
        assert_eq!(rule_org(&notification, &vec!["deraadt".into()]), false)
    }

    #[test]
    fn test_scorer_reason() {
        let notification = create_notification();

        assert_eq!(
            rule_reason(
                &notification,
                &vec!["comment".into(), "participating".into(), "mention".into()]
            ),
            true
        );
        assert_eq!(
            rule_reason(&notification, &vec!["comment".into(), "mention".into()]),
            false
        );
    }
}
