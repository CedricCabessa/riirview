use crate::dirs::Directories;
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone)]
pub struct Config {
    pub github_base_url: String,
    pub db_path: String,
    pub rules_path: PathBuf,
}

static GITHUB_BASE_URL: &str = "https://api.github.com";

impl Default for Config {
    fn default() -> Config {
        Config {
            github_base_url: GITHUB_BASE_URL.to_string(),
            db_path: database_url(),
            rules_path: rules_path(),
        }
    }
}

static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

impl Config {
    pub fn get() -> Config {
        CONFIG
            .get_or_init(|| Mutex::new(Config::default()))
            .lock()
            .unwrap()
            .clone()
    }

    pub fn init_for_test(github_base_url: String, db_path: String, rule_path: String) -> Config {
        let new_config = Config {
            github_base_url,
            db_path,
            rules_path: rule_path.into(),
        };
        let mut config = CONFIG
            .get_or_init(|| Mutex::new(Config::default()))
            .lock()
            .unwrap();
        *config = new_config;
        config.clone()
    }

    pub fn reset() -> Config {
        let mut config = CONFIG
            .get_or_init(|| Mutex::new(Config::default()))
            .lock()
            .unwrap();
        *config = Config::default();
        config.clone()
    }

    pub fn rewrite_url(&self, url: &str) -> String {
        if url.starts_with(GITHUB_BASE_URL) {
            url.replace(GITHUB_BASE_URL, &self.github_base_url)
        } else {
            url.to_string()
        }
    }
}

fn database_url() -> String {
    let directories = Directories::new();
    match dotenvy::var("DATABASE_URL") {
        Ok(val) => val,
        Err(_) => {
            let db_path = directories.data.join("riirview.db");
            db_path.to_str().unwrap().into()
        }
    }
}

fn rules_path() -> PathBuf {
    let directories = Directories::new();
    directories.config.join("rules.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::reset();
        assert_eq!(config.github_base_url, "https://api.github.com");
    }

    #[test]
    fn test_config_init_test() {
        let config = Config::init_for_test(
            "http://localhost:1234".to_string(),
            "/tmp/test.db".to_string(),
            "/tmp/rules.toml".to_string(),
        );
        assert_eq!(config.github_base_url, "http://localhost:1234");
    }

    #[test]
    fn test_config_change() {
        let config = Config::reset();
        assert_eq!(config.github_base_url, "https://api.github.com");
        let config = Config::init_for_test(
            "http://localhost:1234".to_string(),
            "/tmp/test.db".to_string(),
            "/tmp/rules.toml".to_string(),
        );
        assert_eq!(config.github_base_url, "http://localhost:1234");
    }

    #[test]
    fn test_rewrite_url() {
        let test_url = "https://api.github.com/repos/rust-lang/rust";

        let config = Config::reset();
        let rewritten_url = config.rewrite_url(test_url);
        assert_eq!(rewritten_url, test_url);

        let config = Config::init_for_test(
            "http://localhost:1234".to_string(),
            "/tmp/test.db".to_string(),
            "/tmp/rules.toml".to_string(),
        );
        let rewritten_url = config.rewrite_url(test_url);
        assert_eq!(rewritten_url, "http://localhost:1234/repos/rust-lang/rust");
    }
}
