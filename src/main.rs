use clap::Parser;
use url::Url;

use crate::crawler::{Crawler, CrawlerMessage};
use crate::error::CliError;
use crate::rules::CrawlerRules;

mod crawler;
mod error;
mod rules;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    url: Vec<Url>,

    #[arg(short, long, default_value_t = 2)]
    jobs: usize,

    #[arg(short, long, default_value = "crawl/1.0")]
    user_agent: String,

    #[arg(short = 'd', long)]
    max_depth: Option<u32>,

    #[arg(short = 'o', long)]
    max_origin_depth: Option<u32>,

    #[arg(short, long, default_value_t = false)]
    ignore_robots: bool,

    #[arg(short, long, default_value_t = false)]
    parent_dirs: bool,
}

fn main() -> Result<(), CliError> {
    let cli = Cli::parse();

    let rules = CrawlerRules {
        parent_dirs: cli.parent_dirs,
        ignore_robots: cli.ignore_robots,
        max_origin_depth: cli.max_origin_depth,
        max_depth: cli.max_depth,
        roots: &cli.url,
    };

    let on_crawl = |message: CrawlerMessage| {
        match message {
            CrawlerMessage::UrlFound(url) => println!("{url}"),
        }
    };

    let mut crawler = Crawler::new(rules, cli.jobs, on_crawl);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::CreateRuntimeError(e))?;

    runtime.block_on(crawler.start(cli.url.clone()));

    Ok(())
}
