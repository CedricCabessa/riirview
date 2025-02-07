use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

pub struct Directories {
    pub data: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
}

impl Directories {
    pub fn new() -> Directories {
        let dirs = ProjectDirs::from("", "", "riirview").expect("no project dir");
        Directories {
            data: PathBuf::from(dirs.data_dir()),
            config: PathBuf::from(dirs.config_dir()),
            cache: PathBuf::from(dirs.cache_dir()),
        }
    }

    pub fn create(&self) -> Result<()> {
        fs::create_dir_all(&self.data).context(format!("{}", self.data.display()))?;
        fs::create_dir_all(&self.cache).context(format!("{}", self.cache.display()))?;
        fs::create_dir_all(&self.config).context(format!("{}", self.config.display()))?;
        Ok(())
    }
}

impl Default for Directories {
    fn default() -> Self {
        Self::new()
    }
}
