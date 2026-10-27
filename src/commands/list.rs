use anyhow::Result;
use comfy_table::Color;

use super::Env;
use crate::{jar, ui};

pub fn run(env: &Env, json: bool) -> Result<i32> {
    let project = env.project()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&project.config.plugins)?);
        return Ok(0);
    }

    let dir = project.plugins_dir();
    if project.config.plugins.is_empty() {
        ui::info("登録されているプラグインはありません");
    } else {
        let mut table = ui::table(&["名前", "ソース", "ID", "バージョン", "ファイル", ""]);
        for p in &project.config.plugins {
            let exists = dir.join(&p.file).is_file();
            table.add_row(vec![
                ui::colored(&p.name, Color::White),
                ui::source_cell(p.source),
                ui::colored(&p.id, Color::Cyan),
                ui::colored(&p.version, Color::White),
                if exists {
                    ui::colored(&p.file, Color::DarkGrey)
                } else {
                    ui::colored(format!("{} (見つかりません)", p.file), Color::Red)
                },
                if p.pin {
                    ui::colored("固定", Color::DarkYellow)
                } else {
                    ui::colored("", Color::White)
                },
            ]);
        }
        println!("{table}");
        println!(
            "{}",
            ui::dim(format!(
                "{} 件 / {} {}",
                project.config.plugins.len(),
                project.config.server.software,
                project
                    .config
                    .server
                    .minecraft
                    .as_deref()
                    .map(|v| format!("(MC {v})"))
                    .unwrap_or_default()
            ))
        );
    }

    let unregistered = jar::list_jars(&dir)?
        .iter()
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            !project.is_registered_file(&name)
        })
        .count();
    if unregistered > 0 {
        ui::warn(format!(
            "未登録の jar が {unregistered} 個あります → bookshelf plugins register"
        ));
    }
    Ok(0)
}
