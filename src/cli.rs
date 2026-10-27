use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::model::{PluginRef, Software, Source};

#[derive(Parser)]
#[command(
    name = "bookshelf",
    version,
    about = "Paper / Folia サーバー向けプラグイン管理ツール (Modrinth / Hangar / Spigot)"
)]
pub struct Cli {
    /// サーバーのディレクトリ (bookshelf.yml を親方向に探索します)
    #[arg(long, global = true, default_value = ".")]
    pub dir: PathBuf,

    /// 確認プロンプトをすべて承諾する
    #[arg(short, long, global = true)]
    pub yes: bool,

    /// 色付き出力を無効にする (NO_COLOR でも無効化されます)
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// このディレクトリ(サーバー)に bookshelf.yml を作成する
    Init {
        /// サーバーソフトウェア (未指定なら自動検出)
        #[arg(long, value_enum)]
        software: Option<Software>,
        /// Minecraft のバージョン (未指定なら自動検出)
        #[arg(long)]
        mc_version: Option<String>,
        /// plugins ディレクトリ名
        #[arg(long, default_value = "plugins")]
        plugins_dir: String,
        /// 既存の bookshelf.yml を上書きする
        #[arg(long)]
        force: bool,
    },
    /// プラグインの管理
    Plugins {
        #[command(subcommand)]
        command: PluginsCommand,
    },
    /// bookshelf 自身を最新版に更新する
    SelfUpdate,
}

#[derive(Subcommand)]
pub enum PluginsCommand {
    /// Modrinth / Hangar / Spigot からプラグインを検索する
    Search {
        query: String,
        /// 検索対象のソースを絞り込む
        #[arg(short, long, value_enum)]
        source: Option<Source>,
        /// 各ソースの最大件数
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// 登録済みのプラグイン一覧を表示する
    List {
        #[arg(long)]
        json: bool,
    },
    /// plugins ディレクトリの未登録 jar をすべて登録する
    Register,
    /// 登録済みプラグインの更新を確認する
    Check {
        #[arg(long)]
        json: bool,
    },
    /// プラグインを新規導入する (例: modrinth:luckperms)
    Install {
        /// <source>:<id> 形式 (modrinth / hangar / spigot)
        plugin: PluginRef,
    },
    /// プラグインを更新する (名前を省略するとすべて)
    Update { names: Vec<String> },
    /// プラグインを削除して登録を解除する
    Remove { name: String },
}
