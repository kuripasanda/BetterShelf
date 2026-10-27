use anyhow::{Result, bail};

use super::check::{Status, evaluate_all};
use super::{Env, download_release, swap_in};
use crate::model::Download;
use crate::ui;

pub async fn run(env: &Env, names: Vec<String>) -> Result<i32> {
    let mut project = env.project()?;

    let mut indices: Vec<usize> = Vec::new();
    if names.is_empty() {
        indices.extend(0..project.config.plugins.len());
    } else {
        for name in &names {
            match project.find(name) {
                Some(i) => indices.push(i),
                None => bail!("'{name}' は登録されていません (bookshelf plugins list)"),
            }
        }
    }
    if indices.is_empty() {
        ui::info("登録されているプラグインはありません");
        return Ok(0);
    }

    let entries: Vec<_> = indices
        .iter()
        .map(|&i| project.config.plugins[i].clone())
        .collect();
    let spinner = ui::spinner(format!("{} 件の更新を確認中…", entries.len()));
    let statuses = evaluate_all(&env.client, &project.target(), &entries).await;
    spinner.finish_and_clear();

    let (mut updated, mut current, mut manual, mut failed) = (0, 0, 0, 0);
    for ((&idx, entry), status) in indices.iter().zip(&entries).zip(statuses) {
        match status {
            Status::UpToDate => current += 1,
            Status::Pinned => {
                ui::hint(format!("{}: 固定されているためスキップ", entry.name));
            }
            Status::Error(msg) => {
                failed += 1;
                ui::error(format!("{}: {msg}", entry.name));
            }
            Status::Update(release) => {
                if let Download::Manual(page) = &release.download {
                    manual += 1;
                    ui::warn(format!(
                        "{} {}: 自動ダウンロード不可。手動で更新してください → {}",
                        ui::bold(&entry.name),
                        ui::update_arrow(&entry.version, &release.version),
                        ui::cyan(page)
                    ));
                    continue;
                }
                let result = async {
                    let dl = download_release(
                        &env.client,
                        &release,
                        &project.plugins_dir(),
                        &entry.name,
                    )
                    .await?;
                    swap_in(&project, &dl.part, &release.file_name, Some(&entry.file))?;
                    anyhow::Ok(dl.hashes.sha256)
                }
                .await;
                match result {
                    Ok(sha256) => {
                        let target = &mut project.config.plugins[idx];
                        target.version = release.version.clone();
                        target.file = release.file_name.clone();
                        target.sha256 = Some(sha256);
                        project.save()?;
                        updated += 1;
                        ui::success(format!(
                            "{} {}",
                            ui::bold(&entry.name),
                            ui::update_arrow(&entry.version, &release.version)
                        ));
                    }
                    Err(e) => {
                        failed += 1;
                        ui::error(format!("{}: {e:#}", entry.name));
                    }
                }
            }
        }
    }

    println!();
    println!(
        "{} 件更新 / {} 件最新 / {} 件手動対応 / {} 件失敗",
        if updated > 0 {
            ui::green(updated)
        } else {
            updated.to_string()
        },
        current,
        if manual > 0 {
            ui::magenta(manual)
        } else {
            manual.to_string()
        },
        if failed > 0 {
            ui::red(failed)
        } else {
            failed.to_string()
        },
    );
    if updated > 0 {
        ui::hint(
            "旧バージョンは .bookshelf/backup/ に退避しています。サーバーを再起動すると反映されます",
        );
    }
    Ok(if failed > 0 { 1 } else { 0 })
}
