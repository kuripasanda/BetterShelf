use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;

pub const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn client() -> Result<Client> {
    Client::builder()
        .user_agent(format!("bookshelf/{VERSION} ({REPO_URL})"))
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .context("HTTP クライアントの初期化に失敗しました")
}

/// 429 (レート制限) の場合は待機して再試行する
async fn send(client: &Client, url: &str, query: &[(&str, String)]) -> Result<Response> {
    let mut attempt = 0u64;
    loop {
        let resp = client
            .get(url)
            .query(query)
            .send()
            .await
            .with_context(|| format!("{url} への接続に失敗しました"))?;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
            attempt += 1;
            let wait = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(2 * attempt)
                .min(30);
            tokio::time::sleep(Duration::from_secs(wait)).await;
            continue;
        }
        return Ok(resp);
    }
}

pub async fn get_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    query: &[(&str, String)],
) -> Result<T> {
    match get_json_opt(client, url, query).await? {
        Some(v) => Ok(v),
        None => bail!("{url} が見つかりません (404)"),
    }
}

/// 404 のとき `None` を返す
pub async fn get_json_opt<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    query: &[(&str, String)],
) -> Result<Option<T>> {
    let resp = send(client, url, query).await?;
    let status = resp.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        bail!("{url} が HTTP {status} を返しました");
    }
    let value = resp
        .json()
        .await
        .with_context(|| format!("{url} のレスポンスを解析できません"))?;
    Ok(Some(value))
}

pub async fn get_text(client: &Client, url: &str) -> Result<String> {
    let resp = send(client, url, &[]).await?;
    if !resp.status().is_success() {
        bail!("{url} が HTTP {} を返しました", resp.status());
    }
    Ok(resp.text().await?)
}

/// `dest` へストリーミングで保存する (呼び出し側が .part を渡す想定)
pub async fn download(client: &Client, url: &str, dest: &Path, pb: &ProgressBar) -> Result<()> {
    let resp = send(client, url, &[]).await?;
    if !resp.status().is_success() {
        bail!("ダウンロードに失敗しました: HTTP {} ({url})", resp.status());
    }
    if let Some(len) = resp.content_length() {
        pb.set_length(len);
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("{} を作成できません", dest.display()))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("ダウンロード中に通信が切断されました")?;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    file.flush().await?;
    Ok(())
}
