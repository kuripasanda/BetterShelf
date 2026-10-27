use std::path::Path;

use anyhow::Result;

use super::Env;
use crate::config::{PluginEntry, Project};
use crate::model::{SearchHit, Source};
use crate::source::{self, hangar, modrinth};
use crate::{jar, ui};

pub async fn run(env: &Env) -> Result<i32> {
    let mut project = env.project()?;
    let dir = project.plugins_dir();
    let pending: Vec<_> = jar::list_jars(&dir)?
        .into_iter()
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            !project.is_registered_file(&name)
        })
        .collect();

    if pending.is_empty() {
        ui::info("未登録の jar はありません");
        return Ok(0);
    }
    ui::info(format!("{} 個の未登録 jar を照合します", pending.len()));

    let (mut registered, mut skipped) = (0, 0);
    for path in pending {
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        match resolve(env, &project, &path, &file).await {
            Ok(Some(entry)) => {
                ui::success(format!(
                    "{} {} {}",
                    ui::bold(&entry.name),
                    ui::dim(&entry.version),
                    ui::dim(format!("← {}:{}", entry.source, entry.id)),
                ));
                project.config.plugins.push(entry);
                // 途中で中断しても登録済み分が失われないよう都度保存する
                project.save()?;
                registered += 1;
            }
            Ok(None) => {
                ui::warn(format!("{file}: 登録をスキップしました"));
                skipped += 1;
            }
            Err(e) => {
                ui::warn(format!("{file}: {e:#}"));
                skipped += 1;
            }
        }
    }

    println!();
    ui::info(format!(
        "{} 件登録 / {} 件スキップ",
        ui::green(registered),
        if skipped > 0 {
            ui::yellow(skipped)
        } else {
            skipped.to_string()
        }
    ));
    Ok(0)
}

async fn resolve(
    env: &Env,
    project: &Project,
    path: &Path,
    file: &str,
) -> Result<Option<PluginEntry>> {
    let meta = jar::read_meta(path)?;
    let hashes = jar::hashes(path)?;
    if project.find(&meta.name).is_some() {
        ui::warn(format!(
            "{}: 同じ名前のプラグイン '{}' が既に登録されています",
            file, meta.name
        ));
        return Ok(None);
    }
    let jar_version = meta.version.clone().unwrap_or_else(|| "unknown".into());
    let entry = |source: Source, id: String, version: String| PluginEntry {
        name: meta.name.clone(),
        source,
        id,
        version,
        file: file.to_string(),
        sha256: Some(hashes.sha256.clone()),
        pin: false,
    };

    // 1. Modrinth はハッシュで確実に特定できる
    let spinner = ui::spinner(format!("{file}: Modrinth でハッシュ照合中"));
    let identified = modrinth::identify(&env.client, &hashes.sha1).await;
    spinner.finish_and_clear();
    match identified {
        Ok(Some(found)) => return Ok(Some(entry(Source::Modrinth, found.slug, found.version))),
        Ok(None) => {}
        Err(e) => ui::warn(format!("{file}: Modrinth への問い合わせに失敗 ({e:#})")),
    }

    // 2. 名前で全ソースを検索して候補を出す
    let spinner = ui::spinner(format!("{file}: 「{}」を検索中", meta.name));
    let mut candidates: Vec<SearchHit> = Vec::new();
    for src in Source::ALL {
        if let Ok(found) = source::search(&env.client, src, &meta.name, 5).await {
            candidates.extend(found);
        }
    }
    spinner.finish_and_clear();
    // 名前が完全一致する候補を先頭へ (安定ソート)
    candidates.sort_by_key(|h| !h.name.eq_ignore_ascii_case(&meta.name));

    let exact = candidates
        .first()
        .filter(|h| h.name.eq_ignore_ascii_case(&meta.name));
    let chosen = if env.yes || !ui::interactive() {
        match exact {
            Some(hit) => Some(hit.clone()),
            None => {
                ui::hint(format!(
                    "{file}: 名前が一致する候補がないため自動登録できません"
                ));
                None
            }
        }
    } else if candidates.is_empty() {
        None
    } else {
        let mut items: Vec<String> = candidates
            .iter()
            .map(|h| {
                format!(
                    "{} [{}] {} ({} DL)",
                    h.name,
                    h.source.label(),
                    h.reference(),
                    ui::format_count(h.downloads)
                )
            })
            .collect();
        items.push("スキップ".to_string());
        let prompt = format!("{file} (プラグイン名: {}) の登録先を選択", meta.name);
        ui::select(&prompt, &items)
            .filter(|&i| i < candidates.len())
            .map(|i| candidates[i].clone())
    };
    let Some(hit) = chosen else {
        return Ok(None);
    };

    // Hangar は SHA-256 が一致するバージョンがあれば、そのバージョン名を採用する
    let version = if hit.source == Source::Hangar {
        hangar::match_version(&env.client, &hit.id, &hashes.sha256)
            .await
            .ok()
            .flatten()
            .unwrap_or(jar_version)
    } else {
        jar_version
    };
    Ok(Some(entry(hit.source, hit.id, version)))
}
