use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{Env, register};
use crate::config::{Config, Project, ServerConfig, write_config};
use crate::model::Software;
use crate::{jar, ui};

#[derive(Default)]
struct Detected {
    software: Option<Software>,
    minecraft: Option<String>,
}

pub async fn run(
    env: &Env,
    software: Option<Software>,
    mc_version: Option<String>,
    plugins_dir: String,
    force: bool,
) -> Result<i32> {
    let root = env
        .dir
        .canonicalize()
        .with_context(|| format!("{} が見つかりません", env.dir.display()))?;
    let path = Project::file_path(&root);
    if path.exists() && !force {
        bail!(
            "{} は既に存在します。上書きするには --force を指定してください",
            path.display()
        );
    }

    let detected = detect(&root);
    if let Some(s) = detected.software {
        ui::info(format!(
            "サーバーソフトウェアを検出しました: {}",
            ui::bold(s)
        ));
    }
    if let Some(v) = &detected.minecraft {
        ui::info(format!(
            "Minecraft バージョンを検出しました: {}",
            ui::bold(v)
        ));
    }
    let software = software.or(detected.software).unwrap_or(Software::Paper);
    let minecraft = mc_version
        .or(detected.minecraft)
        .or_else(|| ui::input("Minecraft のバージョン (例: 1.21.4 / 空欄で指定なし)"));
    if minecraft.is_none() {
        ui::warn("Minecraft バージョン未指定のため、更新確認でバージョン絞り込みを行いません");
    }

    let config = Config {
        server: ServerConfig {
            software,
            minecraft,
            plugins_dir,
        },
        plugins: vec![],
    };
    write_config(&root, &config)?;
    ui::success(format!(
        "{} を作成しました",
        ui::bold(crate::config::FILE_NAME)
    ));

    let project = Project { root, config };
    let jars = jar::list_jars(&project.plugins_dir())?;
    if jars.is_empty() {
        ui::hint("プラグインを探すには: bookshelf plugins search <キーワード>");
        return Ok(0);
    }
    if ui::confirm(
        &format!("plugins 内の {} 個の jar を今すぐ登録しますか?", jars.len()),
        env.yes,
    ) {
        return register::run(env).await;
    }
    ui::hint("あとで登録するには: bookshelf plugins register");
    Ok(0)
}

fn detect(root: &Path) -> Detected {
    let mut found = Detected::default();

    if let Ok(text) = std::fs::read_to_string(root.join("version_history.json")) {
        found.minecraft = minecraft_from_text(&text);
        if text.contains("Folia") {
            found.software = Some(Software::Folia);
        } else if text.contains("Paper") {
            found.software = Some(Software::Paper);
        }
    }

    // 起動 jar の名前 (paper-1.21.4-123.jar など)
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !name.ends_with(".jar") {
                continue;
            }
            if found.software.is_none() {
                if name.starts_with("folia") {
                    found.software = Some(Software::Folia);
                } else if name.starts_with("paper") {
                    found.software = Some(Software::Paper);
                }
            }
            if found.minecraft.is_none() && (name.starts_with("folia") || name.starts_with("paper"))
            {
                found.minecraft = name
                    .split('-')
                    .find(|p| is_mc_version(p))
                    .map(str::to_string);
            }
        }
    }
    found
}

/// `git-Paper-123 (MC: 1.21.4)` から `1.21.4` を取り出す
fn minecraft_from_text(text: &str) -> Option<String> {
    let rest = text.split("MC: ").nth(1)?;
    let version: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    is_mc_version(&version).then_some(version)
}

fn is_mc_version(s: &str) -> bool {
    s.starts_with("1.") && s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.len() > 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_versions() {
        let text = r#"{"currentVersion":"git-Paper-123 (MC: 1.21.4)"}"#;
        assert_eq!(minecraft_from_text(text).as_deref(), Some("1.21.4"));
        assert_eq!(minecraft_from_text("nothing"), None);
        assert!(is_mc_version("1.21"));
        assert!(!is_mc_version("123"));
        assert!(!is_mc_version("1."));
    }

    #[test]
    fn detects_from_jar_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("folia-1.21.4-10.jar"), b"").unwrap();
        let d = detect(dir.path());
        assert_eq!(d.software, Some(Software::Folia));
        assert_eq!(d.minecraft.as_deref(), Some("1.21.4"));
    }
}
