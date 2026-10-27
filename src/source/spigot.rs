//! SpigotMC は公式 API がないため、サードパーティの Spiget API を使う。
//! SpigotMC 直ホストのファイルは Cloudflare により自動ダウンロードできないので、
//! 外部ホスト(直接 .jar を指す URL)のリソースだけを自動取得の対象にする。

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

use super::safe_file_name;
use crate::http;
use crate::model::{Download, Release, SearchHit, Source};

const API: &str = "https://api.spiget.org/v2";

#[derive(Deserialize)]
struct Resource {
    id: u64,
    name: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    external: bool,
    file: Option<FileObject>,
}

#[derive(Deserialize)]
struct FileObject {
    #[serde(rename = "externalUrl")]
    external_url: Option<String>,
}

#[derive(Deserialize)]
struct LatestVersion {
    #[serde(default)]
    name: String,
}

pub fn page_url(id: &str) -> String {
    format!("https://www.spigotmc.org/resources/{id}/")
}

pub async fn search(client: &Client, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let url = format!("{API}/search/resources/{}", urlencoding::encode(query));
    let resources: Vec<Resource> = http::get_json(
        client,
        &url,
        &[
            ("field", "name".to_string()),
            ("size", limit.to_string()),
            ("sort", "-downloads".to_string()),
        ],
    )
    .await?;
    Ok(resources
        .into_iter()
        .map(|r| SearchHit {
            source: Source::Spigot,
            id: r.id.to_string(),
            name: r.name,
            description: r.tag,
            downloads: r.downloads,
        })
        .collect())
}

pub async fn latest(client: &Client, id: &str) -> Result<Release> {
    let resource: Resource = http::get_json(client, &format!("{API}/resources/{id}"), &[]).await?;
    let version: LatestVersion = http::get_json(
        client,
        &format!("{API}/resources/{id}/versions/latest"),
        &[],
    )
    .await?;

    let direct_jar = resource
        .file
        .and_then(|f| f.external_url)
        .filter(|u| resource.external && is_jar_url(u));
    let (download, file_name) = match direct_jar {
        Some(url) => {
            let name = safe_file_name(url.split(['?', '#']).next().unwrap_or(""), &resource.name);
            (Download::Url(url), name)
        }
        None => (
            Download::Manual(page_url(id)),
            safe_file_name("", &resource.name),
        ),
    };
    Ok(Release {
        version: version.name,
        file_name,
        download,
        sha1: None,
        sha256: None,
        sha512: None,
    })
}

fn is_jar_url(url: &str) -> bool {
    url.starts_with("https://")
        && url
            .split(['?', '#'])
            .next()
            .is_some_and(|p| p.to_ascii_lowercase().ends_with(".jar"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jar_url_detection() {
        assert!(is_jar_url("https://example.com/a/Foo-1.0.jar"));
        assert!(is_jar_url("https://example.com/Foo.jar?x=1"));
        assert!(!is_jar_url("https://example.com/releases"));
        assert!(!is_jar_url("http://example.com/Foo.jar"));
    }
}
