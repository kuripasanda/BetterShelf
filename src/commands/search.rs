use anyhow::{Result, bail};
use comfy_table::Color;
use futures_util::future::join_all;

use super::{Env, install};
use crate::config::Project;
use crate::model::{PluginRef, SearchHit, Source};
use crate::{source, ui};

pub async fn run(
    env: &Env,
    query: &str,
    only: Option<Source>,
    limit: usize,
    json: bool,
) -> Result<i32> {
    let sources: Vec<Source> = match only {
        Some(s) => vec![s],
        None => Source::ALL.to_vec(),
    };

    let spinner = (!json).then(|| ui::spinner(format!("「{query}」を検索中…")));
    let results = join_all(
        sources
            .iter()
            .map(|&s| async move { (s, source::search(&env.client, s, query, limit).await) }),
    )
    .await;
    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    let mut hits: Vec<SearchHit> = Vec::new();
    let mut failed = 0;
    for (src, result) in results {
        match result {
            Ok(found) => hits.extend(found),
            Err(e) => {
                failed += 1;
                ui::warn(format!("{}: 検索に失敗しました ({e:#})", src.label()));
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
        return Ok(0);
    }
    if hits.is_empty() {
        if failed == sources.len() {
            bail!("すべてのソースで検索に失敗しました");
        }
        ui::info(format!(
            "「{query}」に一致するプラグインは見つかりませんでした"
        ));
        return Ok(0);
    }

    let mut table = ui::table(&["#", "名前", "ソース", "インストール指定", "DL数", "説明"]);
    for (i, hit) in hits.iter().enumerate() {
        table.add_row(vec![
            ui::colored(i + 1, Color::DarkGrey),
            ui::colored(&hit.name, Color::White),
            ui::source_cell(hit.source),
            ui::colored(hit.reference(), Color::Cyan),
            ui::colored(ui::format_count(hit.downloads), Color::White),
            ui::colored(ui::truncate(&hit.description, 48), Color::DarkGrey),
        ]);
    }
    println!("{table}");

    // bookshelf.yml があり対話端末なら、そのままインストールへ進める
    if ui::interactive() && Project::load(&env.dir).is_ok() {
        let mut items: Vec<String> = hits
            .iter()
            .map(|h| format!("{} [{}] {}", h.name, h.source.label(), h.reference()))
            .collect();
        items.push("何もしない".to_string());
        if let Some(i) = ui::select("インストールするプラグインを選択", &items)
            && i < hits.len()
        {
            let plugin = PluginRef {
                source: hits[i].source,
                id: hits[i].id.clone(),
            };
            return install::run(env, plugin).await;
        }
    } else {
        ui::hint("インストール: bookshelf plugins install <インストール指定>");
    }
    Ok(0)
}
