use crate::gh::Notification;
use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(PartialEq, Eq, Debug)]
enum RuleType {
    Author,
    Participating,
    Repo,
    Title,
}

#[derive(Deserialize, Debug)]
struct TomlRule {
    rule: String,
    param: String,
    score: i32,
}

#[derive(Debug)]
struct Rule {
    rule: RuleType,
    name: String,
    params: Vec<String>,
    score: i32,
}

impl Rule {
    pub fn matcher(&self, notification: &Notification) -> i32 {
        let fct = match self.rule {
            RuleType::Author => rule_author,
            RuleType::Participating => rule_participating,
            RuleType::Repo => rule_repo,
            RuleType::Title => rule_title,
        };
        if fct(notification, &self.params) {
            debug!("{} match {}", notification.title(), self.name);
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
    pub fn new(toml_path: &str) -> Result<Scorer, Box<dyn std::error::Error>> {
        let config = fs::read_to_string(toml_path)?;

        //let value = config.parse::<HashMap<String, Rule>>();
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
            .fold(0, |acc, rule| acc + rule.matcher(&notification))
    }
}

fn rule_from_str(rule_name: &str) -> Result<RuleType, String> {
    match rule_name {
        "author" => Ok(RuleType::Author),
        "participating" => Ok(RuleType::Participating),
        "repo" => Ok(RuleType::Repo),
        "title" => Ok(RuleType::Title),
        _ => Err(format!("Unknown rule name: {}", rule_name).into()),
    }
}

fn rule_author(_notification: &Notification, _params: &Vec<String>) -> bool {
    false
}

fn rule_participating(_notification: &Notification, _params: &Vec<String>) -> bool {
    false
}

fn rule_repo(notification: &Notification, params: &Vec<String>) -> bool {
    params.contains(notification.repo())
}

fn rule_title(notification: &Notification, params: &Vec<String>) -> bool {
    params.iter().any(|p| notification.title().contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_scorer_builder() {
        let path = "tests/rules.toml";
        let scorer = Scorer::new(path).unwrap();

        assert_eq!(scorer.rules.len(), 4);
        let display_names: HashSet<String> = scorer.rules.iter().map(|r| r.name.clone()).collect();
        assert_eq!(
            display_names,
            HashSet::from([
                "me".into(),
                "participating".into(),
                "friends".into(),
                "my_fav_repos".into(),
            ])
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
}
