mod cli;
mod commands;
mod config;
mod http;
mod jar;
mod model;
mod source;
mod ui;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    ui::init(cli.no_color);
    match commands::run(cli).await {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            ui::error(format!("{err:#}"));
            std::process::exit(1);
        }
    }
}
