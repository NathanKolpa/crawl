use clap::{App, Arg};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::header::USER_AGENT;
use reqwest::{Body, Client};
use select::document::Document;
use select::predicate::Name;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::error::Error;
use std::option::Option::{None, Some};
use std::pin::Pin;
use std::result::Result::{Err, Ok};
use std::sync::{Arc, Mutex};
use url::Url;

struct QueuedLink {
    url: Url,
    depth: i32,
    origin_depth: i32,
}

fn get_links(base: &Url, html: &str) -> Vec<Url> {
    let mut res = vec![];

    let dom = Document::from(html);

    for anchor in dom.find(Name("a")) {
        if let Some(href) = anchor.attr("href") {
            let url = if let Ok(url) = Url::parse(href) {
                Some(url)
            } else if let Ok(url) = base.join(href) {
                Some(url)
            } else {
                None
            };

            if let Some(url) = url {
                res.push(url);
            }
        }
    }

    res
}

async fn crawl_link(
    crawled_list: &Arc<Mutex<HashSet<Url>>>,
    link_queue: &Arc<Mutex<VecDeque<QueuedLink>>>,
    user_agent: &str,
    max_depth: Option<i32>,
    max_origin_depth: Option<i32>,
    client: &Client,
    current: QueuedLink,
) {
    {
        let mut crawled_list = crawled_list.lock().unwrap();
        crawled_list.insert(current.url.clone());
    }

    println!("{}", current.url);

    let res = client
        .get(current.url.clone())
        .header(USER_AGENT, user_agent)
        .send()
        .await;

    if let Err(err) = res {
        eprintln!("error while fetching: {}", err);
    } else if let Ok(res) = res {

        if let Some(content_type) = res.headers().get("Content-Type") {
            if content_type != "text/html" && content_type != "application/html" {
                return;
            }
        }
        else {
            return;
        }

        let body = res.text().await;

        if let Err(err) = body {
            eprintln!("error while receiving body: {}", err);
        } else if let Ok(body) = body {
            let new_links = get_links(&current.url, &body);

            let crawled_list = crawled_list.lock().unwrap();
            let mut link_queue = link_queue.lock().unwrap();

            for new_link in new_links {
                if crawled_list.contains(&new_link) {
                    continue;
                }

                let origin_depth = if new_link.origin() != current.url.origin() {
                    current.origin_depth + 1
                } else {
                    current.origin_depth
                };

                let depth = current.depth + 1;

                if let Some(max_depth) = max_depth {
                    if depth > max_depth {
                        continue;
                    }
                }

                if let Some(max_origin_depth) = max_origin_depth {
                    if origin_depth > max_origin_depth {
                        continue;
                    }
                }

                link_queue.push_back(QueuedLink {
                    origin_depth,
                    url: new_link,
                    depth,
                })
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("crawl")
        .version(clap::crate_version!())
        .author("Nathan Kolpa <nathan@kolpa.me>")
        .about("A simple cli webcrawler.")
        .arg(
            Arg::with_name("url")
                .value_name("URL")
                .help("The start url")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("user-agent")
                .short("u")
                .long("user-agent")
                .takes_value(true)
                .default_value("Crawl")
                .required(false)
                .help("The user agent that will be sent along with every request"),
        )
        .arg(
            Arg::with_name("max-depth")
                .short("d")
                .long("max-depth")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("max-origin-depth")
                .short("o")
                .long("max-origin-depth")
                .takes_value(true)
                .required(false)
                .default_value("1"),
        )
        .arg(
            Arg::with_name("jobs")
                .takes_value(true)
                .required(false)
                .short("j")
                .long("jobs")
                .help("The maximum amount of parallel jobs.")
                .default_value("8"),
        )
        .get_matches();

    let start = Url::parse(matches.value_of("url").unwrap())?;

    let user_agent = matches.value_of("user-agent").unwrap_or("Crawl");

    let max_depth: Option<i32> = if let Some(depth) = matches.value_of("max-depth") {
        Some(depth.parse()?)
    } else {
        None
    };

    let max_origin_depth: Option<i32> = if let Some(depth) = matches.value_of("max-origin-depth") {
        Some(depth.parse()?)
    } else {
        None
    };

    let max_jobs = if let Some(depth) = matches.value_of("jobs") {
        depth.parse()?
    } else {
        8
    };

    //

    let crawled_list: Arc<Mutex<HashSet<Url>>> = Default::default();
    let link_queue: Arc<Mutex<VecDeque<QueuedLink>>> =
        Arc::new(Mutex::new(VecDeque::from(vec![QueuedLink {
            url: start,
            depth: 1,
            origin_depth: 1,
        }])));

    let client = reqwest::Client::new();
    let mut jobs = FuturesUnordered::new();
    let mut job_count = 0;

    'outer: loop {
        while job_count >= max_jobs || (link_queue.lock().unwrap().len() <= 0 && job_count > 0) {
            jobs.next().await;
            job_count -= 1;
        }

        for _ in job_count..max_jobs {
            let current = {
                let mut link_queue = link_queue.lock().unwrap();
                link_queue.pop_front()
            };

            match current {
                None => {
                    if job_count <= 0 {
                        break 'outer;
                    }
                }
                Some(current) => {
                    jobs.push(crawl_link(
                        &crawled_list,
                        &link_queue,
                        &user_agent,
                        max_depth,
                        max_origin_depth,
                        &client,
                        current,
                    ));

                    job_count += 1;
                }
            }
        }
    }

    Ok(())
}
