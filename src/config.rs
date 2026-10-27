use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::{Software, Source, Target};

pub const FILE_NAME: &str = "bookshelf.yml";
const HEADER: &str = "# bookshelf が管理するファイルです。手動で編集しても構いません。\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub software: Software,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft: Option<String>,
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: String,
}

fn default_plugins_dir() -> String {
    "plugins".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub name: String,
    pub source: Source,
    pub id: String,
    pub version: String,
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pin: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// bookshelf.yml とそのあるディレクトリ(サーバールート)
#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub config: Config,
}

impl Project {
    /// `dir` から親方向へ bookshelf.yml を探す
    pub fn load(dir: &Path) -> Result<Self> {
        let start = dir
            .canonicalize()
            .with_context(|| format!("{} が見つかりません", dir.display()))?;
        for candidate in start.ancestors() {
            let path = candidate.join(FILE_NAME);
            if path.is_file() {
                return Self::load_file(&path);
            }
        }
        bail!(
            "{FILE_NAME} が見つかりません。サーバーのディレクトリで `bookshelf init` を実行してください"
        )
    }

    fn load_file(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("{} を読み込めません", path.display()))?;
        let config: Config = serde_yaml::from_str(&text)
            .with_context(|| format!("{} の形式が不正です", path.display()))?;
        Ok(Project {
            root: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            config,
        })
    }

    pub fn file_path(root: &Path) -> PathBuf {
        root.join(FILE_NAME)
    }

    pub fn save(&self) -> Result<()> {
        write_config(&self.root, &self.config)
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.root.join(&self.config.server.plugins_dir)
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.root.join(".bookshelf").join("backup")
    }

    pub fn target(&self) -> Target {
        Target {
            software: self.config.server.software,
            minecraft: self.config.server.minecraft.clone(),
        }
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.config
            .plugins
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn is_registered_file(&self, file: &str) -> bool {
        self.config.plugins.iter().any(|p| p.file == file)
    }
}

pub fn write_config(root: &Path, config: &Config) -> Result<()> {
    let body = serde_yaml::to_string(config)?;
    let path = Project::file_path(root);
    // 書き込み途中で壊さないよう一時ファイル経由で置き換える
    let tmp = path.with_extension("yml.tmp");
    std::fs::write(&tmp, format!("{HEADER}{body}"))
        .with_context(|| format!("{} に書き込めません", tmp.display()))?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let config = Config {
            server: ServerConfig {
                software: Software::Paper,
                minecraft: Some("1.21.4".into()),
                plugins_dir: "plugins".into(),
            },
            plugins: vec![PluginEntry {
                name: "LuckPerms".into(),
                source: Source::Modrinth,
                id: "luckperms".into(),
                version: "5.4".into(),
                file: "LuckPerms.jar".into(),
                sha256: Some("abc".into()),
                pin: false,
            }],
        };
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), &config).unwrap();
        let loaded = Project::load(dir.path()).unwrap();
        assert_eq!(loaded.config.plugins.len(), 1);
        assert_eq!(loaded.config.plugins[0].version, "5.4");
        assert_eq!(loaded.config.server.minecraft.as_deref(), Some("1.21.4"));
        assert!(loaded.find("luckperms").is_some());
    }

    #[test]
    fn load_searches_parents() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            server: ServerConfig {
                software: Software::Folia,
                minecraft: None,
                plugins_dir: "plugins".into(),
            },
            plugins: vec![],
        };
        write_config(dir.path(), &config).unwrap();
        let sub = dir.path().join("plugins");
        std::fs::create_dir(&sub).unwrap();
        let loaded = Project::load(&sub).unwrap();
        assert_eq!(loaded.config.server.software, Software::Folia);
    }
}
