mod check;
mod init;
mod install;
mod list;
mod register;
mod remove;
mod search;
mod self_update;
mod update;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use reqwest::Client;

use crate::cli::{Cli, Command, PluginsCommand};
use crate::config::{PluginEntry, Project};
use crate::model::{Download, Release};
use crate::{http, jar, ui};

pub struct Env {
    pub dir: PathBuf,
    pub yes: bool,
    pub client: Client,
}

impl Env {
    pub fn project(&self) -> Result<Project> {
        Project::load(&self.dir)
    }
}

/// 終了コードを返す
pub async fn run(cli: Cli) -> Result<i32> {
    let env = Env {
        dir: cli.dir,
        yes: cli.yes,
        client: http::client()?,
    };
    match cli.command {
        Command::Init {
            software,
            mc_version,
            plugins_dir,
            force,
        } => init::run(&env, software, mc_version, plugins_dir, force).await,
        Command::SelfUpdate => self_update::run(&env).await,
        Command::Plugins { command } => match command {
            PluginsCommand::Search {
                query,
                source,
                limit,
                json,
            } => search::run(&env, &query, source, limit, json).await,
            PluginsCommand::List { json } => list::run(&env, json),
            PluginsCommand::Register => register::run(&env).await,
            PluginsCommand::Check { json } => check::run(&env, json).await,
            PluginsCommand::Install { plugin } => install::run(&env, plugin).await,
            PluginsCommand::Update { names } => update::run(&env, names).await,
            PluginsCommand::Remove { name } => remove::run(&env, &name),
        },
    }
}

/// "v1.2" と "1.2" のような表記ゆれを吸収して比較する
pub fn same_version(a: &str, b: &str) -> bool {
    fn norm(s: &str) -> String {
        s.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase()
    }
    norm(a) == norm(b)
}

pub fn is_up_to_date(entry: &PluginEntry, release: &Release) -> bool {
    if let (Some(a), Some(b)) = (&entry.sha256, &release.sha256)
        && a.eq_ignore_ascii_case(b)
    {
        return true;
    }
    same_version(&entry.version, &release.version)
}

pub struct Downloaded {
    pub part: PathBuf,
    pub hashes: jar::Hashes,
}

/// `<dir>/<file>.part` にダウンロードし、取得できるハッシュで検証する
pub async fn download_release(
    client: &Client,
    release: &Release,
    dir: &Path,
    label: &str,
) -> Result<Downloaded> {
    let Download::Url(url) = &release.download else {
        bail!("このリリースは自動ダウンロードできません");
    };
    tokio::fs::create_dir_all(dir)
        .await
        .with_context(|| format!("{} を作成できません", dir.display()))?;
    let part = dir.join(format!("{}.part", release.file_name));
    let pb = ui::download_bar(label.to_string());
    let result = http::download(client, url, &part, &pb).await;
    pb.finish_and_clear();
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }

    let hashes = match jar::hashes(&part) {
        Ok(h) => h,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    let checks = [
        ("SHA-256", &release.sha256, &hashes.sha256),
        ("SHA-1", &release.sha1, &hashes.sha1),
        ("SHA-512", &release.sha512, &hashes.sha512),
    ];
    for (algo, expected, actual) in checks {
        if let Some(expected) = expected
            && !expected.eq_ignore_ascii_case(actual)
        {
            let _ = std::fs::remove_file(&part);
            bail!(
                "{algo} が一致しません (期待: {expected} / 実際: {actual})。ダウンロードを破棄しました"
            );
        }
    }
    if jar::read_meta(&part).is_err() {
        let _ = std::fs::remove_file(&part);
        bail!("ダウンロードしたファイルがプラグイン jar ではありません");
    }
    Ok(Downloaded { part, hashes })
}

/// `.bookshelf/backup/<name>.<unix時刻>` へ移動する
pub fn backup_file(project: &Project, path: &Path) -> Result<PathBuf> {
    let backup_dir = project.backup_dir();
    std::fs::create_dir_all(&backup_dir)?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let dest = backup_dir.join(format!("{name}.{ts}"));
    std::fs::rename(path, &dest)
        .with_context(|| format!("{} をバックアップできません", path.display()))?;
    Ok(dest)
}

/// .part を本来のファイル名へ置き換える。`old_file` があればバックアップへ退避し、失敗時は元に戻す
pub fn swap_in(
    project: &Project,
    part: &Path,
    new_file: &str,
    old_file: Option<&str>,
) -> Result<()> {
    let dir = project.plugins_dir();
    let new_path = dir.join(new_file);
    let old_path = old_file.map(|f| dir.join(f));

    if new_path.exists() && old_path.as_deref() != Some(new_path.as_path()) {
        let _ = std::fs::remove_file(part);
        bail!("{new_file} は既に存在するため上書きできません");
    }

    let mut backup: Option<(PathBuf, PathBuf)> = None;
    if let Some(old_path) = old_path.filter(|p| p.exists()) {
        let dest = backup_file(project, &old_path)?;
        backup = Some((old_path, dest));
    }

    if let Err(e) = std::fs::rename(part, &new_path) {
        if let Some((old_path, dest)) = backup {
            let _ = std::fs::rename(dest, old_path);
        }
        let _ = std::fs::remove_file(part);
        return Err(e).with_context(|| format!("{new_file} を配置できません"));
    }
    Ok(())
}
