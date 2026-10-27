use std::fmt;
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Source {
    Modrinth,
    Hangar,
    Spigot,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Modrinth, Source::Hangar, Source::Spigot];

    pub fn label(self) -> &'static str {
        match self {
            Source::Modrinth => "Modrinth",
            Source::Hangar => "Hangar",
            Source::Spigot => "Spigot",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label().to_lowercase())
    }
}

impl FromStr for Source {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "modrinth" => Ok(Source::Modrinth),
            "hangar" => Ok(Source::Hangar),
            "spigot" | "spigotmc" => Ok(Source::Spigot),
            other => Err(format!(
                "不明なソース '{other}' です (modrinth / hangar / spigot)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Software {
    Paper,
    Folia,
}

impl fmt::Display for Software {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Software::Paper => "paper",
            Software::Folia => "folia",
        })
    }
}

/// `source:id` 形式の指定 (例: `modrinth:luckperms`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRef {
    pub source: Source,
    pub id: String,
}

impl FromStr for PluginRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (source, id) = s.split_once(':').ok_or_else(|| {
            format!("'{s}' は <source>:<id> 形式で指定してください (例: modrinth:luckperms)")
        })?;
        let id = id.trim();
        if id.is_empty() {
            return Err("ID が空です".into());
        }
        Ok(PluginRef {
            source: source.parse()?,
            id: id.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub source: Source,
    pub id: String,
    pub name: String,
    pub description: String,
    pub downloads: u64,
}

impl SearchHit {
    pub fn reference(&self) -> String {
        format!("{}:{}", self.source, self.id)
    }
}

#[derive(Debug, Clone)]
pub enum Download {
    /// 直接ダウンロードできる URL
    Url(String),
    /// 自動ダウンロード不可。ブラウザで開くページ URL
    Manual(String),
}

/// 各ソースの「最新リリース」を共通化した構造
#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub file_name: String,
    pub download: Download,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub sha512: Option<String>,
}

/// 検索・更新確認時の絞り込み条件
#[derive(Debug, Clone)]
pub struct Target {
    pub software: Software,
    pub minecraft: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plugin_ref() {
        let r: PluginRef = "modrinth:luckperms".parse().unwrap();
        assert_eq!(r.source, Source::Modrinth);
        assert_eq!(r.id, "luckperms");
        assert!("luckperms".parse::<PluginRef>().is_err());
        assert!("foo:bar".parse::<PluginRef>().is_err());
        assert!("hangar:".parse::<PluginRef>().is_err());
    }
}
