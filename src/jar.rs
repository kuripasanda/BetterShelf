use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};

#[derive(Debug, Clone)]
pub struct Meta {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Hashes {
    pub sha1: String,
    pub sha256: String,
    pub sha512: String,
}

pub fn list_jars(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut jars = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(jars),
        Err(e) => return Err(e).with_context(|| format!("{} を読み込めません", dir.display())),
    };
    for entry in entries {
        let path = entry?.path();
        let is_jar = path.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jar"));
        if is_jar {
            jars.push(path);
        }
    }
    jars.sort();
    Ok(jars)
}

pub fn hashes(path: &Path) -> Result<Hashes> {
    let mut file = File::open(path).with_context(|| format!("{} を開けません", path.display()))?;
    let (mut h1, mut h256, mut h512) = (Sha1::new(), Sha256::new(), Sha512::new());
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h1.update(&buf[..n]);
        h256.update(&buf[..n]);
        h512.update(&buf[..n]);
    }
    Ok(Hashes {
        sha1: hex::encode(h1.finalize()),
        sha256: hex::encode(h256.finalize()),
        sha512: hex::encode(h512.finalize()),
    })
}

/// plugin.yml / paper-plugin.yml から名前とバージョンを読む
pub fn read_meta(path: &Path) -> Result<Meta> {
    let file = File::open(path).with_context(|| format!("{} を開けません", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("{} は有効な jar ではありません", path.display()))?;

    let mut name = None;
    let mut version = None;
    for entry in ["plugin.yml", "paper-plugin.yml"] {
        let Ok(mut f) = archive.by_name(entry) else {
            continue;
        };
        let mut text = String::new();
        if f.read_to_string(&mut text).is_err() {
            continue;
        }
        let (n, v) = parse_plugin_yml(&text);
        name = name.or(n);
        version = version.or(v);
    }
    match name {
        Some(name) => Ok(Meta { name, version }),
        None => bail!(
            "{} に plugin.yml / paper-plugin.yml がありません",
            path.display()
        ),
    }
}

fn parse_plugin_yml(text: &str) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return (None, None);
    };
    let get = |key: &str| {
        value.get(key).and_then(|v| match v {
            serde_yaml::Value::String(s) => Some(s.trim().to_string()),
            serde_yaml::Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
    };
    (
        get("name").filter(|s| !s.is_empty()),
        get("version").filter(|s| !s.is_empty()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_jar(path: &Path, entries: &[(&str, &str)]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, body) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn reads_plugin_yml() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("a.jar");
        make_jar(
            &jar,
            &[("plugin.yml", "name: Foo\nversion: 1.2\nmain: x.Y\n")],
        );
        let meta = read_meta(&jar).unwrap();
        assert_eq!(meta.name, "Foo");
        assert_eq!(meta.version.as_deref(), Some("1.2"));
    }

    #[test]
    fn falls_back_to_paper_plugin_yml() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("b.jar");
        make_jar(
            &jar,
            &[("paper-plugin.yml", "name: Bar\nversion: '2.0.1'\n")],
        );
        let meta = read_meta(&jar).unwrap();
        assert_eq!(meta.name, "Bar");
        assert_eq!(meta.version.as_deref(), Some("2.0.1"));
    }

    #[test]
    fn rejects_non_plugin_jar() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("c.jar");
        make_jar(&jar, &[("readme.txt", "hi")]);
        assert!(read_meta(&jar).is_err());
    }

    #[test]
    fn computes_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.jar");
        std::fs::write(&f, b"abc").unwrap();
        let h = hashes(&f).unwrap();
        assert_eq!(h.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            h.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
