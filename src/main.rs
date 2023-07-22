use clap::Parser;
use reqwest::ClientBuilder;
use std::error::Error;
use url::Url;

use crate::crawler::Crawler;
use crate::error::{CliError, CrawlerError};
use crate::queue::CrawlerQueue;
use crate::rules::CrawlerRules;
use crate::scheduler::{CrawlerMessage, Scheduler};

mod crawler;
mod error;
mod queue;
mod rules;
mod scheduler;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(required = true)]
    url: Vec<Url>,

    /// Specify the concurrently running jobs.
    /// Running multiple jobs comes with little overhead and no additional threads are created.
    /// This value should respect the maximum amount of connections specified by the OS.
    #[arg(short, long, default_value_t = 1)]
    jobs: usize,

    /// The user agent sent along with every request and used for checking against the "robots.txt" file.
    #[arg(short, long, default_value = "crawl/1.1.0")]
    user_agent: String,

    /// Specify if the crawler should ignore the "robots.txt" file for each website.
    #[arg(short, long, default_value_t = false)]
    ignore_robots: bool,

    /// Allow crawling of non subdirectory urls.
    /// Please note, using this flag can cause the program to try to scan the entire internet.
    #[arg(short, long, default_value_t = false)]
    allow_non_subdirectories: bool,

    #[arg(short = 'd', long)]
    max_depth: Option<u32>,

    #[arg(short = 'o', long)]
    max_origin_depth: Option<u32>,

    /// Print out all found urls that would otherwise be ignored.
    #[arg(short, long, default_value_t = false)]
    non_conforming: bool,
}

fn main() -> Result<(), CliError> {
    let cli = Cli::parse();

    let rules = CrawlerRules {
        only_subdirs: !cli.allow_non_subdirectories,
        roots: &cli.url,
    };

    let client = ClientBuilder::new()
        .user_agent(cli.user_agent)
        .build()
        .unwrap();

    let crawler = Crawler::new(rules, &client);
    let queue = CrawlerQueue::new(false, cli.max_origin_depth, cli.max_depth);

    let on_crawl = |message: CrawlerMessage| match message {
        CrawlerMessage::UrlFound(url) => println!("{url}"),
        CrawlerMessage::Error(err) => {
            if err.is_cli_relevant() {
                eprintln!("{err}")
            }
        }
    };

    let mut scheduler = Scheduler::new(queue, crawler, cli.jobs, on_crawl);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::CreateRuntimeError(e))?;

    runtime.block_on(scheduler.start(cli.url.clone()));

    Ok(())
}
