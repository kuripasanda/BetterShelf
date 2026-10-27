use anyhow::{Result, bail};

use super::{Env, backup_file};
use crate::ui;

pub fn run(env: &Env, name: &str) -> Result<i32> {
    let mut project = env.project()?;
    let Some(idx) = project.find(name) else {
        bail!("'{name}' は登録されていません (bookshelf plugins list)");
    };
    let entry = project.config.plugins[idx].clone();

    if !ui::confirm(
        &format!("{} ({}) を削除しますか?", entry.name, entry.file),
        env.yes,
    ) {
        ui::info("キャンセルしました (非対話環境では --yes が必要です)");
        return Ok(1);
    }

    let path = project.plugins_dir().join(&entry.file);
    if path.is_file() {
        let backup = backup_file(&project, &path)?;
        let shown = backup.strip_prefix(&project.root).unwrap_or(&backup);
        ui::hint(format!("jar は {} に退避しました", shown.display()));
    } else {
        ui::warn(format!("{} は既に存在しません", entry.file));
    }
    project.config.plugins.remove(idx);
    project.save()?;
    ui::success(format!("{} を削除しました", ui::bold(&entry.name)));
    Ok(0)
}
