use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

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

    pub fn create(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.data)?;
        fs::create_dir_all(&self.cache)?;
        fs::create_dir_all(&self.config)?;
        Ok(())
    }
}

impl Default for Directories {
    fn default() -> Self {
        Self::new()
    }
}
