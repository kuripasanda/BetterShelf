use anyhow::Result;
use comfy_table::Color;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;

use super::{Env, is_up_to_date};
use crate::config::PluginEntry;
use crate::model::{Download, Release, Target};
use crate::{source, ui};

pub enum Status {
    UpToDate,
    Update(Release),
    Pinned,
    Error(String),
}

pub async fn evaluate(client: &Client, target: &Target, entry: &PluginEntry) -> Status {
    if entry.pin {
        return Status::Pinned;
    }
    match source::latest(client, entry.source, &entry.id, target).await {
        Ok(release) if is_up_to_date(entry, &release) => Status::UpToDate,
        Ok(release) => Status::Update(release),
        Err(e) => Status::Error(format!("{e:#}")),
    }
}

/// 全エントリを並行して確認する (結果は入力順)
pub async fn evaluate_all(
    client: &Client,
    target: &Target,
    entries: &[PluginEntry],
) -> Vec<Status> {
    futures_util::stream::iter(entries)
        .map(|e| evaluate(client, target, e))
        .buffered(6)
        .collect()
        .await
}

#[derive(Serialize)]
struct Row<'a> {
    name: &'a str,
    source: String,
    current: &'a str,
    latest: Option<&'a str>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

pub async fn run(env: &Env, json: bool) -> Result<i32> {
    let project = env.project()?;
    let entries = &project.config.plugins;
    if entries.is_empty() {
        if json {
            println!("[]");
        } else {
            ui::info("登録されているプラグインはありません (bookshelf plugins register)");
        }
        return Ok(0);
    }

    let spinner = (!json).then(|| ui::spinner(format!("{} 件の更新を確認中…", entries.len())));
    let statuses = evaluate_all(&env.client, &project.target(), entries).await;
    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    let (mut updates, mut errors, mut latest_count) = (0, 0, 0);
    let mut manual: Vec<(&str, &str)> = Vec::new();
    let mut failures: Vec<(&str, &str)> = Vec::new();
    let mut rows = Vec::new();
    let mut table = ui::table(&["名前", "ソース", "現在", "最新", "状態"]);

    for (entry, status) in entries.iter().zip(&statuses) {
        let (latest, state, cell, url, error) = match status {
            Status::UpToDate => {
                latest_count += 1;
                (
                    Some(entry.version.as_str()),
                    "up-to-date",
                    ui::colored("✔ 最新", Color::Green),
                    None,
                    None,
                )
            }
            Status::Pinned => (
                None,
                "pinned",
                ui::colored("● 固定", Color::DarkGrey),
                None,
                None,
            ),
            Status::Update(r) => {
                updates += 1;
                match &r.download {
                    Download::Url(_) => (
                        Some(r.version.as_str()),
                        "update",
                        ui::colored("↑ 更新あり", Color::Yellow),
                        None,
                        None,
                    ),
                    Download::Manual(page) => {
                        manual.push((&entry.name, page));
                        (
                            Some(r.version.as_str()),
                            "manual",
                            ui::colored("! 手動で更新", Color::Magenta),
                            Some(page.as_str()),
                            None,
                        )
                    }
                }
            }
            Status::Error(msg) => {
                errors += 1;
                failures.push((&entry.name, msg));
                (
                    None,
                    "error",
                    ui::colored("✘ エラー", Color::Red),
                    None,
                    Some(msg.as_str()),
                )
            }
        };
        table.add_row(vec![
            ui::colored(&entry.name, Color::White),
            ui::source_cell(entry.source),
            ui::colored(&entry.version, Color::White),
            match (state, latest) {
                ("update" | "manual", Some(v)) => ui::colored(v, Color::Green),
                (_, Some(v)) => ui::colored(v, Color::DarkGrey),
                _ => ui::colored("-", Color::DarkGrey),
            },
            cell,
        ]);
        rows.push(Row {
            name: &entry.name,
            source: entry.source.to_string(),
            current: &entry.version,
            latest,
            status: state,
            url,
            error,
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(if errors > 0 { 1 } else { 0 });
    }

    println!("{table}");
    for (name, page) in &manual {
        ui::warn(format!("{name}: 自動ダウンロード不可 → {page}"));
    }
    for (name, msg) in &failures {
        ui::error(format!("{name}: {msg}"));
    }
    println!(
        "{} 件更新あり / {} 件最新 / {} 件エラー",
        if updates > 0 {
            ui::yellow(updates)
        } else {
            updates.to_string()
        },
        ui::green(latest_count),
        if errors > 0 {
            ui::red(errors)
        } else {
            errors.to_string()
        },
    );
    if updates > manual.len() {
        ui::hint("更新するには: bookshelf plugins update");
    }
    Ok(if errors > 0 { 1 } else { 0 })
}
