use clap::{App, Arg, ArgMatches};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::header::USER_AGENT;
use reqwest::{Body, Client, Response};
use select::predicate::Name;
use std::collections::{HashMap, HashSet};
use std::collections::VecDeque;
use std::error::Error;
use std::fmt::Debug;
use select::document::Document;
use std::option::Option::{None, Some};
use robotstxt::DefaultMatcher;
use std::result::Result::{Err, Ok};
use std::sync::{Arc, Mutex};
use url::{Origin, Url};

struct QueuedLink {
    url: Url,
    depth: i32,
    origin_depth: i32,
}

struct SiteData {
    robots_body: Option<String>
}

fn get_links(base: &Url, html: &str) -> HashSet<Url> {
    let mut urls: HashSet<Url> = Default::default();

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
                urls.insert(url);
            }
        }
    }

    urls
}

fn is_correct_content_type(res: &Response) -> bool {
    if let Some(content_type) = res.headers().get("Content-Type") {
        if let Ok(content_type) = content_type.to_str() {
            if !content_type.starts_with("text/html") && content_type.starts_with("application/html") {
                return false;
            }
        }
    } else {
        return false;
    }

    true
}

async fn crawl_link(
    crawled_list: &Arc<Mutex<HashSet<Url>>>,
    link_queue: &Arc<Mutex<VecDeque<QueuedLink>>>,
    site_data: &Arc<Mutex<HashMap<Origin, SiteData>>>,
    start_url: &Url,
    user_agent: &str,
    max_depth: Option<i32>,
    max_origin_depth: Option<i32>,
    send_head: bool,
    only_subdirs: bool,
    respect_robots: bool,
    client: &Client,
    current: QueuedLink,
) {
    if respect_robots {
        let origin = current.url.origin();
        let mut site_data = site_data.lock().unwrap();

        if !site_data.contains_key(&origin) {
            let robots_url = Url::parse(&origin.ascii_serialization()).unwrap().join("robots.txt").unwrap();

            let res = client
                .get(robots_url)
                .header(USER_AGENT, user_agent)
                .send()
                .await;

            let current_site_data = if let Ok(res) = res {// TODO: error logging
                let body = res.text().await;

                if let Ok(body) = body {
                    SiteData {
                        robots_body: Some(body)
                    }
                }
                else {
                    SiteData {
                        robots_body: None
                    }
                }
            } else {
                SiteData {
                    robots_body: None
                }
            };

            site_data.insert(origin.clone(), current_site_data);
        }

        let site_data = site_data.get(&origin).unwrap();
        let mut matcher = DefaultMatcher::default();

        if let Some(robots_body) = &site_data.robots_body {
            if !matcher.one_agent_allowed_by_robots(robots_body, user_agent, &current.url.to_string()) {
                return;
            }
        }
    }

    println!("{}", current.url);

    if send_head {
        let head_res = client
            .head(current.url.clone())
            .header(USER_AGENT, user_agent)
            .send()
            .await;

        if let Ok(head_res) = head_res {
            if !is_correct_content_type(&head_res) {
                return;
            }
        }
    }

    let res = client
        .get(current.url.clone())
        .header(USER_AGENT, user_agent)
        .send()
        .await;

    if let Err(err) = res {
        eprintln!("error while fetching: {}", err);
        return;
    }

    if let Ok(res) = res {

        if !is_correct_content_type(&res) {
            return;
        }

        let body = res.text().await;

        if let Err(err) = body {
            eprintln!("error while receiving body: {}", err);
            return;
        }

        if let Ok(body) = body {
            let new_links = get_links(&current.url, &body);

            let mut crawled_list = crawled_list.lock().unwrap();
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

                if only_subdirs {
                    if new_link.origin() != current.url.origin() || !new_link.path().starts_with(start_url.path()) {
                        continue
                    }
                }

                link_queue.push_back(QueuedLink {
                    origin_depth,
                    url: new_link,
                    depth,
                });

                crawled_list.insert(current.url.clone());
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
                .default_value("crawl")
                .required(false)
                .value_name("text")
                .help("The User-Agent header that will be sent along with every request"),
        )
        .arg(
            Arg::with_name("max-depth")
                .short("d")
                .long("max-depth")
                .takes_value(true)
                .value_name("number")
                .required(false),
        )
        .arg(
            Arg::with_name("max-origin-depth")
                .short("o")
                .long("max-origin-depth")
                .takes_value(true)
                .required(false)
                .value_name("number")
                .default_value("1"),
        )
        .arg(
            Arg::with_name("jobs")
                .takes_value(true)
                .required(false)
                .short("j")
                .long("jobs")
                .value_name("number")
                .help("The maximum amount of parallel jobs.")
                .default_value("8"),
        )
        .arg(
            Arg::with_name("send-head")
                .required(false)
                .takes_value(true)
                .short("h")
                .long("head")
                .help("Send a HEAD request first before sending a GET request")
                .default_value("true")
                .value_name("boolean")
                .possible_values(&["true", "false"])
        )
        .arg(
            Arg::with_name("respect-robots")
                .required(false)
                .takes_value(true)
                .short("r")
                .long("respect-robots")
                .help("Respect robots.txt")
                .default_value("true")
                .value_name("boolean")
                .possible_values(&["true", "false"])
        )
        .arg(
            Arg::with_name("only-subdirs")
                .required(false)
                .takes_value(true)
                .short("s")
                .long("only-subdirs")
                .help("Crawl only sub directories from the URL")
                .default_value("false")
                .value_name("boolean")
                .possible_values(&["true", "false"])
        )
        .get_matches();

    let start = Url::parse(matches.value_of("url").unwrap())?;

    let user_agent = matches.value_of("user-agent").unwrap_or("crawl");

    let max_depth: Option<i32> = if let Some(depth) = matches.value_of("max-depth") {
        if depth == "none" {
            None
        }
        else {
            Some(depth.parse()?)
        }
    } else {
        None
    };

    let max_origin_depth: Option<i32> = if let Some(depth) = matches.value_of("max-origin-depth") {
        if depth == "none" {
            None
        }
        else {
            Some(depth.parse()?)
        }
    } else {
        None
    };

    let max_jobs = if let Some(depth) = matches.value_of("jobs") {
        depth.parse()?
    } else {
        8
    };

    let send_head = get_bool_arg("send-head", &matches, true);
    let respect_robots = get_bool_arg("repsect-robots", &matches, true);
    let only_subdirs  = get_bool_arg("only-subdirs", &matches, false);

    //

    let crawled_list: Arc<Mutex<HashSet<Url>>> = Default::default();
    let site_data: Arc<Mutex<HashMap<Origin, SiteData>>> = Default::default();
    let link_queue: Arc<Mutex<VecDeque<QueuedLink>>> =
        Arc::new(Mutex::new(VecDeque::from(vec![QueuedLink {
            url: start.clone(),
            depth: 1,
            origin_depth: 1,
        }])));

    let client = reqwest::Client::new();
    let mut jobs = FuturesUnordered::new();
    let mut job_count = 0;

    'outer: loop {
        while job_count >= max_jobs || (link_queue.lock().unwrap().len() <= 0 && job_count > 0) {
            jobs.next().await; // does not remove this future from jobs
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
                        &site_data,
                        &start,
                        &user_agent,
                        max_depth,
                        max_origin_depth,
                        send_head,
                        only_subdirs,
                        respect_robots,
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

fn get_bool_arg(name: &str, matches: &ArgMatches, default: bool) -> bool {
    if let Some(send_head) = matches.value_of(name) {
        if send_head == "true" {
            true
        } else {
            false
        }
    } else {
        default
    }
}
