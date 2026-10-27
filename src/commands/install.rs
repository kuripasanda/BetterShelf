use anyhow::{Result, bail};

use super::{Env, download_release, swap_in};
use crate::config::PluginEntry;
use crate::model::{Download, PluginRef};
use crate::{jar, source, ui};

pub async fn run(env: &Env, plugin: PluginRef) -> Result<i32> {
    let mut project = env.project()?;

    let spinner = ui::spinner(format!("{} の最新版を取得中…", plugin.id));
    let release = source::latest(&env.client, plugin.source, &plugin.id, &project.target()).await;
    spinner.finish_and_clear();
    let release = release?;

    if let Download::Manual(page) = &release.download {
        ui::warn(format!(
            "{}:{} は自動ダウンロードできません。次のページから手動で入手してください",
            plugin.source, plugin.id
        ));
        println!("  {}", ui::cyan(page));
        return Ok(1);
    }

    let dl = download_release(
        &env.client,
        &release,
        &project.plugins_dir(),
        &release.file_name,
    )
    .await?;
    let meta = jar::read_meta(&dl.part)?;
    if project.find(&meta.name).is_some() {
        let _ = std::fs::remove_file(&dl.part);
        bail!(
            "'{}' は既に登録されています。更新するには: bookshelf plugins update {}",
            meta.name,
            meta.name
        );
    }

    swap_in(&project, &dl.part, &release.file_name, None)?;
    project.config.plugins.push(PluginEntry {
        name: meta.name.clone(),
        source: plugin.source,
        id: plugin.id,
        version: release.version.clone(),
        file: release.file_name.clone(),
        sha256: Some(dl.hashes.sha256),
        pin: false,
    });
    project.save()?;
    ui::success(format!(
        "{} {} を導入しました",
        ui::bold(&meta.name),
        ui::green(&release.version)
    ));
    ui::hint("サーバーを再起動すると反映されます");
    Ok(0)
}
