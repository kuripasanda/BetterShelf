//! GitHub Releases から bookshelf 自身を更新する。
//! リリースには `bookshelf-linux-<arch>` (生バイナリ) と `bookshelf-linux-<arch>.sha256` を添付する。

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::Env;
use crate::{http, jar, ui};

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub async fn run(env: &Env) -> Result<i32> {
    if !cfg!(target_os = "linux") {
        bail!("self-update は Linux のみ対応しています");
    }
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("未対応のアーキテクチャです: {other}"),
    };
    let asset_name = format!("bookshelf-linux-{arch}");
    let repo = http::REPO_URL
        .trim_end_matches('/')
        .trim_start_matches("https://github.com/");

    let spinner = ui::spinner("最新バージョンを確認中…");
    let release: Result<GithubRelease> = http::get_json(
        &env.client,
        &format!("https://api.github.com/repos/{repo}/releases/latest"),
        &[],
    )
    .await;
    spinner.finish_and_clear();
    let release = release?;

    let latest = release.tag_name.trim_start_matches('v');
    if !is_newer(latest, http::VERSION) {
        ui::success(format!("既に最新です (v{})", http::VERSION));
        return Ok(0);
    }
    ui::info(format!(
        "新しいバージョンがあります: {}",
        ui::update_arrow(http::VERSION, latest)
    ));

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .with_context(|| format!("リリースに {asset_name} が見つかりません"))?;

    let exe = std::env::current_exe().context("実行ファイルの場所を特定できません")?;
    let part = exe.with_extension("update.part");
    let pb = ui::download_bar(asset_name.clone());
    let result = http::download(&env.client, &asset.browser_download_url, &part, &pb).await;
    pb.finish_and_clear();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e).context("書き込み権限がない場合は sudo で実行してください");
    }

    // チェックサムが公開されていれば検証する
    match release
        .assets
        .iter()
        .find(|a| a.name == format!("{asset_name}.sha256"))
    {
        Some(sum) => {
            let text = http::get_text(&env.client, &sum.browser_download_url).await?;
            let expected = text.split_whitespace().next().unwrap_or_default();
            let actual = jar::hashes(&part)?.sha256;
            if !expected.eq_ignore_ascii_case(&actual) {
                let _ = std::fs::remove_file(&part);
                bail!("SHA-256 が一致しません。更新を中止しました");
            }
        }
        None => ui::warn("チェックサムが公開されていないため検証できませんでした"),
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&part, &exe).with_context(|| {
        format!(
            "{} を置き換えられません (権限を確認してください)",
            exe.display()
        )
    })?;
    ui::success(format!("v{latest} に更新しました"));
    Ok(0)
}

fn parse_version(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.0"));
    }
}
