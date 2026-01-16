use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageCacheEntry {
    pub installed: bool,
    pub last_checked: DateTime<Utc>,
    pub check_command: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PackageCacheData {
    pub version: u32,
    // Key format: "profile_name::package_name"
    pub entries: HashMap<String, PackageCacheEntry>,
}

#[derive(Debug)]
pub struct PackageCache {
    cache_file: PathBuf,
    data: PackageCacheData,
}

impl Default for PackageCache {
    fn default() -> Self {
        // Try to load from default location, otherwise fall back to empty memory-only cache (which won't save correctly if path is bad, but satisfies trait)
        match Self::new() {
            Ok(cache) => cache,
            Err(e) => {
                warn!(
                    "Failed to initialize package cache with default path: {}",
                    e
                );
                // Fallback to a dummy path that probably won't write successfully but allows the app to validly construct the struct.
                // Or better: use a sensible default path even if we couldn't create it right now.
                let config_dir = crate::utils::get_config_dir();
                Self {
                    cache_file: config_dir.join("package_status.json"),
                    data: PackageCacheData::default(),
                }
            }
        }
    }
}

impl PackageCache {
    pub fn new() -> Result<Self> {
        let config_dir = crate::utils::get_config_dir();
        let cache_file = config_dir.join("package_status.json");

        let data = if cache_file.exists() {
            match std::fs::read_to_string(&cache_file) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("Failed to parse package cache: {}", e);
                        PackageCacheData::default()
                    }
                },
                Err(e) => {
                    warn!("Failed to read package cache: {}", e);
                    PackageCacheData::default()
                }
            }
        } else {
            PackageCacheData::default()
        };

        Ok(Self { cache_file, data })
    }

    fn get_key(profile_name: &str, package_name: &str) -> String {
        format!("{}::{}", profile_name, package_name)
    }

    pub fn get_status(&self, profile_name: &str, package_name: &str) -> Option<&PackageCacheEntry> {
        self.data
            .entries
            .get(&Self::get_key(profile_name, package_name))
    }

    pub fn update_status(
        &mut self,
        profile_name: &str,
        package_name: &str,
        installed: bool,
        check_command: Option<String>,
        output: Option<String>,
    ) -> Result<()> {
        let key = Self::get_key(profile_name, package_name);

        let entry = PackageCacheEntry {
            installed,
            last_checked: Utc::now(),
            check_command,
            output,
        };

        self.data.entries.insert(key, entry);
        self.save()
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.cache_file.parent() {
            std::fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let json = serde_json::to_string_pretty(&self.data)
            .context("Failed to serialize package cache")?;

        std::fs::write(&self.cache_file, json).context("Failed to write package cache file")?;

        debug!("Package cache saved to {:?}", self.cache_file);
        Ok(())
    }
}
