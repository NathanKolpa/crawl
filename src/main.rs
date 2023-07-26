use clap::Parser;
use crawl::*;
use url::Url;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(required = true)]
    urls: Vec<Url>,

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

pub enum CliError {
    CreateRuntimeError(std::io::Error),
}

impl std::fmt::Debug for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::CreateRuntimeError(e) => write!(f, "Cannot create tokio runtime: {e}"),
        }
    }
}

async fn run(args: Cli) -> Result<(), CliError> {
    let on_crawl = |message: CrawlerMessage| match message {
        CrawlerMessage::UrlFound(url) => println!("{url}"),
        CrawlerMessage::Error(err) => eprintln!("{err}"),
    };

    let mut crawler = Crawler::new(
        &args.user_agent,
        false,
        !args.allow_non_subdirectories,
        args.max_depth,
        args.max_origin_depth,
        args.jobs,
        on_crawl,
    );

    crawler.start(args.urls).await;

    Ok(())
}

fn main() -> Result<(), CliError> {
    let cli = Cli::parse();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(CliError::CreateRuntimeError)?;

    runtime.block_on(run(cli))
}
