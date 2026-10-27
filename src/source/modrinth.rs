use anyhow::{Result, bail};
use reqwest::Client;
use serde::Deserialize;

use super::safe_file_name;
use crate::http;
use crate::model::{Download, Release, SearchHit, Software, Source, Target};

const API: &str = "https://api.modrinth.com/v2";

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    downloads: u64,
}

pub async fn search(client: &Client, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let facets =
        r#"[["categories:paper","categories:folia","categories:spigot","categories:bukkit"]]"#;
    let resp: SearchResponse = http::get_json(
        client,
        &format!("{API}/search"),
        &[
            ("query", query.to_string()),
            ("limit", limit.to_string()),
            ("facets", facets.to_string()),
            ("index", "relevance".to_string()),
        ],
    )
    .await?;
    Ok(resp
        .hits
        .into_iter()
        .map(|h| SearchHit {
            source: Source::Modrinth,
            id: h.slug,
            name: h.title,
            description: h.description,
            downloads: h.downloads,
        })
        .collect())
}

#[derive(Deserialize)]
struct Version {
    version_number: String,
    #[serde(default)]
    version_type: String,
    files: Vec<VersionFile>,
}

#[derive(Deserialize)]
struct VersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    hashes: FileHashes,
}

#[derive(Deserialize)]
struct FileHashes {
    sha1: Option<String>,
    sha512: Option<String>,
}

fn loaders(software: Software) -> &'static str {
    match software {
        Software::Paper => r#"["paper","spigot","bukkit"]"#,
        Software::Folia => r#"["folia","paper","spigot","bukkit"]"#,
    }
}

pub async fn latest(client: &Client, id: &str, target: &Target) -> Result<Release> {
    let mut query = vec![("loaders", loaders(target.software).to_string())];
    if let Some(mc) = &target.minecraft {
        query.push(("game_versions", format!("[\"{mc}\"]")));
    }
    let url = format!("{API}/project/{}/version", urlencoding::encode(id));
    let versions: Vec<Version> = http::get_json(client, &url, &query).await?;

    // 新しい順。正式版(release)を優先し、なければ beta/alpha も許容
    let version = versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first());
    let Some(version) = version else {
        match &target.minecraft {
            Some(mc) => bail!("Minecraft {mc} に対応するバージョンがありません"),
            None => bail!("対応するバージョンがありません"),
        }
    };
    let Some(file) = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
    else {
        bail!(
            "バージョン {} にファイルがありません",
            version.version_number
        );
    };
    Ok(Release {
        version: version.version_number.clone(),
        file_name: safe_file_name(&file.filename, id),
        download: Download::Url(file.url.clone()),
        sha1: file.hashes.sha1.clone(),
        sha256: None,
        sha512: file.hashes.sha512.clone(),
    })
}

pub struct Identified {
    pub slug: String,
    pub version: String,
}

#[derive(Deserialize)]
struct VersionInfo {
    project_id: String,
    version_number: String,
}

#[derive(Deserialize)]
struct ProjectInfo {
    slug: String,
}

/// jar の SHA-1 から Modrinth 上のプロジェクトを特定する
pub async fn identify(client: &Client, sha1: &str) -> Result<Option<Identified>> {
    let info: Option<VersionInfo> = http::get_json_opt(
        client,
        &format!("{API}/version_file/{sha1}"),
        &[("algorithm", "sha1".to_string())],
    )
    .await?;
    let Some(info) = info else {
        return Ok(None);
    };
    let project: ProjectInfo =
        http::get_json(client, &format!("{API}/project/{}", info.project_id), &[]).await?;
    Ok(Some(Identified {
        slug: project.slug,
        version: info.version_number,
    }))
}
