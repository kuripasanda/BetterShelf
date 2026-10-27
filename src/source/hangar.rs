use std::collections::HashMap;

use anyhow::{Result, bail};
use reqwest::Client;
use serde::Deserialize;

use super::safe_file_name;
use crate::http;
use crate::model::{Download, Release, SearchHit, Source, Target};

const API: &str = "https://hangar.papermc.io/api/v1";
const SITE: &str = "https://hangar.papermc.io";

#[derive(Deserialize)]
struct ProjectsResponse {
    result: Vec<Project>,
}

#[derive(Deserialize)]
struct Project {
    name: String,
    namespace: Namespace,
    #[serde(default)]
    description: String,
    #[serde(default)]
    stats: Stats,
}

#[derive(Deserialize)]
struct Namespace {
    slug: String,
}

#[derive(Deserialize, Default)]
struct Stats {
    #[serde(default)]
    downloads: u64,
}

pub async fn search(client: &Client, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let resp: ProjectsResponse = http::get_json(
        client,
        &format!("{API}/projects"),
        &[
            ("q", query.to_string()),
            ("limit", limit.to_string()),
            ("offset", "0".to_string()),
        ],
    )
    .await?;
    Ok(resp
        .result
        .into_iter()
        .map(|p| SearchHit {
            source: Source::Hangar,
            id: p.namespace.slug,
            name: p.name,
            description: p.description,
            downloads: p.stats.downloads,
        })
        .collect())
}

#[derive(Deserialize)]
struct VersionsResponse {
    result: Vec<Version>,
}

#[derive(Deserialize)]
struct Version {
    name: String,
    #[serde(default)]
    downloads: HashMap<String, DownloadInfo>,
}

#[derive(Deserialize)]
struct DownloadInfo {
    #[serde(rename = "fileInfo")]
    file_info: Option<FileInfo>,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(rename = "externalUrl")]
    external_url: Option<String>,
}

#[derive(Deserialize)]
struct FileInfo {
    name: Option<String>,
    #[serde(rename = "sha256Hash")]
    sha256_hash: Option<String>,
}

async fn versions(
    client: &Client,
    slug: &str,
    channel: Option<&str>,
    minecraft: Option<&str>,
    limit: usize,
) -> Result<Vec<Version>> {
    let mut query = vec![
        ("limit", limit.to_string()),
        ("offset", "0".to_string()),
        ("platform", "PAPER".to_string()),
    ];
    if let Some(c) = channel {
        query.push(("channel", c.to_string()));
    }
    if let Some(mc) = minecraft {
        query.push(("platformVersion", mc.to_string()));
    }
    let url = format!("{API}/projects/{}/versions", urlencoding::encode(slug));
    let resp: VersionsResponse = http::get_json(client, &url, &query).await?;
    Ok(resp.result)
}

pub async fn latest(client: &Client, slug: &str, target: &Target) -> Result<Release> {
    let mc = target.minecraft.as_deref();
    // 正式版チャンネルを優先し、なければ全チャンネルから探す
    let mut found = versions(client, slug, Some("Release"), mc, 1).await?;
    if found.is_empty() {
        found = versions(client, slug, None, mc, 1).await?;
    }
    let Some(version) = found.into_iter().next() else {
        match mc {
            Some(mc) => bail!("Minecraft {mc} に対応するバージョンがありません"),
            None => bail!("バージョンが見つかりません"),
        }
    };
    let Some(dl) = version.downloads.get("PAPER") else {
        bail!(
            "バージョン {} に PAPER 向けファイルがありません",
            version.name
        );
    };
    let file_name = dl
        .file_info
        .as_ref()
        .and_then(|f| f.name.clone())
        .unwrap_or_default();
    let download = match (&dl.download_url, &dl.external_url) {
        (Some(url), _) => Download::Url(url.clone()),
        (None, Some(_)) => Download::Manual(format!(
            "{SITE}/{}/versions/{}",
            slug,
            urlencoding::encode(&version.name)
        )),
        (None, None) => Download::Url(format!(
            "{API}/projects/{}/versions/{}/PAPER/download",
            urlencoding::encode(slug),
            urlencoding::encode(&version.name)
        )),
    };
    Ok(Release {
        file_name: safe_file_name(&file_name, slug),
        version: version.name,
        download,
        sha1: None,
        sha256: dl.file_info.as_ref().and_then(|f| f.sha256_hash.clone()),
        sha512: None,
    })
}

/// 直近のバージョンの中から SHA-256 が一致するものを探し、そのバージョン名を返す
pub async fn match_version(client: &Client, slug: &str, sha256: &str) -> Result<Option<String>> {
    let found = versions(client, slug, None, None, 50).await?;
    Ok(found
        .into_iter()
        .find(|v| {
            v.downloads
                .get("PAPER")
                .and_then(|d| d.file_info.as_ref())
                .and_then(|f| f.sha256_hash.as_deref())
                .is_some_and(|h| h.eq_ignore_ascii_case(sha256))
        })
        .map(|v| v.name))
}
