pub mod hangar;
pub mod modrinth;
pub mod spigot;

use anyhow::Result;
use reqwest::Client;

use crate::model::{Release, SearchHit, Source, Target};

pub async fn search(
    client: &Client,
    source: Source,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    match source {
        Source::Modrinth => modrinth::search(client, query, limit).await,
        Source::Hangar => hangar::search(client, query, limit).await,
        Source::Spigot => spigot::search(client, query, limit).await,
    }
}

/// 対象サーバーに合う最新リリースを返す。見つからなければエラー
pub async fn latest(client: &Client, source: Source, id: &str, target: &Target) -> Result<Release> {
    match source {
        Source::Modrinth => modrinth::latest(client, id, target).await,
        Source::Hangar => hangar::latest(client, id, target).await,
        Source::Spigot => spigot::latest(client, id).await,
    }
}

/// ダウンロード URL 等から安全なファイル名を作る (パス区切りを除去し .jar を保証)
pub fn safe_file_name(name: &str, fallback: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('.');
    let base = if base.is_empty() { fallback } else { base };
    if base.to_ascii_lowercase().ends_with(".jar") {
        base.to_string()
    } else {
        format!("{base}.jar")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(safe_file_name("../../etc/passwd", "x"), "passwd.jar");
        assert_eq!(safe_file_name("Foo-1.0.jar", "x"), "Foo-1.0.jar");
        assert_eq!(safe_file_name("..\\evil.JAR", "x"), "evil.JAR");
        assert_eq!(safe_file_name("", "fallback"), "fallback.jar");
    }
}
