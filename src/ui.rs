//! 色・表・スピナー・対話プロンプトの共通ヘルパー。
//! 色の意味: 緑=成功/最新, 黄=更新あり, 赤=エラー, マゼンタ=警告/手動対応, シアン=情報

use std::fmt::Display;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use dialoguer::theme::{ColorfulTheme, SimpleTheme, Theme};
use dialoguer::{Confirm, Select};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

use crate::model::Source;

static COLOR: AtomicBool = AtomicBool::new(false);

pub fn init(no_color: bool) {
    let on = !no_color && std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();
    COLOR.store(on, Ordering::Relaxed);
}

pub fn color() -> bool {
    COLOR.load(Ordering::Relaxed)
}

pub fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

macro_rules! painter {
    ($($name:ident => $method:ident),* $(,)?) => {
        $(pub fn $name(s: impl Display) -> String {
            if color() { s.$method().to_string() } else { s.to_string() }
        })*
    };
}

painter! {
    green => green,
    yellow => yellow,
    red => red,
    cyan => cyan,
    magenta => magenta,
    dim => dimmed,
    bold => bold,
}

pub fn success(msg: impl Display) {
    println!("{} {msg}", green("✔"));
}

pub fn info(msg: impl Display) {
    println!("{} {msg}", cyan("ℹ"));
}

pub fn warn(msg: impl Display) {
    println!("{} {msg}", magenta("!"));
}

pub fn error(msg: impl Display) {
    eprintln!("{} {msg}", red("✘"));
}

pub fn hint(msg: impl Display) {
    println!("  {}", dim(msg));
}

pub fn update_arrow(old: &str, new: &str) -> String {
    format!("{} {} {}", red(old), yellow("→"), green(new))
}

fn source_color(source: Source) -> Color {
    match source {
        Source::Modrinth => Color::Green,
        Source::Hangar => Color::Blue,
        Source::Spigot => Color::Rgb {
            r: 255,
            g: 140,
            b: 0,
        },
    }
}

pub fn source_cell(source: Source) -> Cell {
    Cell::new(source.label()).fg(source_color(source))
}

pub fn header(cols: &[&str]) -> Vec<Cell> {
    cols.iter()
        .map(|c| Cell::new(c).add_attribute(Attribute::Bold).fg(Color::Cyan))
        .collect()
}

pub fn table(cols: &[&str]) -> Table {
    let mut t = Table::new();
    t.load_style(UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(header(cols));
    if !color() {
        t.force_no_tty();
    }
    t
}

pub fn colored(text: impl Display, color: Color) -> Cell {
    Cell::new(text).fg(color)
}

pub fn spinner(msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"]),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

pub fn download_bar(msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template(
            "{msg:<28} {bar:30.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec})",
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    pb.set_message(msg.into());
    pb
}

fn theme() -> Box<dyn Theme> {
    if color() {
        Box::new(ColorfulTheme::default())
    } else {
        Box::new(SimpleTheme)
    }
}

/// `--yes` なら常に true、非対話端末なら false
pub fn confirm(prompt: &str, yes: bool) -> bool {
    if yes {
        return true;
    }
    if !interactive() {
        return false;
    }
    Confirm::with_theme(&*theme())
        .with_prompt(prompt)
        .default(false)
        .interact()
        .unwrap_or(false)
}

/// 選択肢から1つ選ぶ。Esc / 非対話端末では None
pub fn select(prompt: &str, items: &[String]) -> Option<usize> {
    if !interactive() {
        return None;
    }
    Select::with_theme(&*theme())
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact_opt()
        .ok()
        .flatten()
}

/// 1行入力。空入力・非対話端末では None
pub fn input(prompt: &str) -> Option<String> {
    if !interactive() {
        return None;
    }
    let text: String = dialoguer::Input::with_theme(&*theme())
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()
        .ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn truncate(s: &str, max: usize) -> String {
    let s = s.replace(['\n', '\r'], " ");
    if s.chars().count() <= max {
        s
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

pub fn format_count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}K", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_and_count() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
        assert_eq!(format_count(1_500), "1.5K");
        assert_eq!(format_count(2_000_000), "2.0M");
    }
}
