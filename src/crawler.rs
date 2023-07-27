use std::pin::Pin;

use futures::future::{join_all, select_all};
use futures::join;
use reqwest::{Client, ClientBuilder};
use tokio::sync::mpsc;
use url::Url;

use crate::error::CrawlerError;
use crate::fetching::LinkFetcher;
use crate::filter::{CrawledUrl, UrlFilter, UrlFilterRules};
use crate::queue::CrawlerQueue;

pub enum CrawlerMessage<'a> {
    UrlFound(&'a Url),
    Error(CrawlerError),
}

enum MasterMessage {
    NewUrl(CrawledUrl),
}

enum WorkerMessage {
    Success(Vec<CrawledUrl>),
    Failure(CrawlerError),
}

struct WorkerHandle {
    tx: mpsc::Sender<MasterMessage>,
    is_working: bool,
}

/// Handles all concurrency and synchronization beween crawling jobs.
pub struct Crawler<T> {
    http_client: Client,
    queue: CrawlerQueue,
    callback: T,
    max_jobs: usize,
    filter: UrlFilterRules,
}

impl<T> Crawler<T> {
    pub fn new(
        user_agent: &str,
        require_robots: bool,
        only_subdirs: bool,
        max_origin_depth: Option<u32>,
        max_depth: Option<u32>,
        max_jobs: usize,
        callback: T,
    ) -> Self {
        Self {
            filter: UrlFilterRules { only_subdirs },
            http_client: ClientBuilder::new().user_agent(user_agent).build().unwrap(),
            queue: CrawlerQueue::new(require_robots, max_origin_depth, max_depth),
            callback,
            max_jobs,
        }
    }
}

impl<T: FnMut(CrawlerMessage)> Crawler<T> {
    async fn start_master(
        &mut self,
        mut workers: Vec<WorkerHandle>,
        mut worker_channel: mpsc::Receiver<(usize, WorkerMessage)>,
        rules: UrlFilter<'_>,
    ) {
        let mut active_workers = 0;

        // TODO: len() does not give the correct behaviour
        while !self.queue.is_empty() || active_workers > 0 {
            let next_url = self.queue.take();
            let has_next_url = next_url.is_some();

            // Dispatch a new job if there is work to do.
            if let Some(url) = next_url {
                // Do a linear scan for a free worker.
                let free_worker = workers.iter_mut().find(|x| !x.is_working);

                match free_worker {
                    None => {} // Oh no, a labour shortage!
                    Some(worker) => {
                        active_workers += 1;
                        worker.is_working = true;
                        (self.callback)(CrawlerMessage::UrlFound(&url.url));
                        worker.tx.send(MasterMessage::NewUrl(url)).await.unwrap();
                    }
                }
            }

            // Dispatching a new job won't do anything, instead we wait for messages.
            if (!has_next_url || active_workers == self.max_jobs) && active_workers != 0 {
                let (i, message) = worker_channel.recv().await.unwrap();

                workers[i].is_working = false;
                active_workers -= 1;

                match message {
                    WorkerMessage::Success(mut urls) => {
                        urls.retain(|url| rules.matches(&url.url));
                        self.queue.check_and_queue_iter(urls.into_iter());
                    }
                    WorkerMessage::Failure(e) => (self.callback)(CrawlerMessage::Error(e)),
                }
            }
        }
    }

    async fn start_worker(
        id: usize,
        http_client: &Client,
        tx: mpsc::Sender<(usize, WorkerMessage)>,
        mut rx: mpsc::Receiver<MasterMessage>,
    ) {
        let link_fetcher = LinkFetcher::new(http_client);

        loop {
            match rx.recv().await {
                None => break,
                Some(MasterMessage::NewUrl(url)) => {
                    let result = link_fetcher.fetch_links(&url).await;

                    match result {
                        Ok(urls) => tx.send((id, WorkerMessage::Success(urls))).await.unwrap(),
                        Err(e) => tx.send((id, WorkerMessage::Failure(e))).await.unwrap(),
                    }
                }
            }
        }
    }

    pub async fn start(&mut self, roots: Vec<Url>) {
        self.queue
            .check_and_queue_iter(roots.clone().into_iter().map(|url| CrawledUrl {
                url,
                depth: 0,
                origin_depth: 0,
            }));

        let client = self.http_client.clone();

        let mut worker_futures = Vec::with_capacity(self.max_jobs);
        let mut worker_channels = Vec::with_capacity(self.max_jobs);
        let (worker_tx, worker_rx) = mpsc::channel::<(usize, WorkerMessage)>(1);

        for i in 0..self.max_jobs {
            let (master_tx, master_rx) = mpsc::channel::<MasterMessage>(1);

            worker_futures.push(Self::start_worker(i, &client, worker_tx.clone(), master_rx));

            worker_channels.push(WorkerHandle {
                is_working: false,
                tx: master_tx,
            });
        }

        let worker_join_handle = join_all(worker_futures);
        let master_handle = self.start_master(
            worker_channels,
            worker_rx,
            UrlFilter::new(self.filter.clone(), &roots),
        );

        join!(worker_join_handle, master_handle);
    }
}
