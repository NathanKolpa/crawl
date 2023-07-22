use futures::future::{join_all, select_all};
use futures::join;
use tokio::sync::mpsc;
use url::Url;

use crate::crawler::{Crawler, CrawlerError};
use crate::queue::CrawlerQueue;
use crate::rules::CrawledUrl;

pub enum CrawlerMessage<'a> {
    UrlFound(&'a Url),
    Error(&'a Url, CrawlerError),
}

enum MasterMessage {
    NewUrl(CrawledUrl),
}

enum WorkerMessage {
    Success(Vec<CrawledUrl>),
    Failure(CrawlerError, Url),
}

struct WorkerHandle {
    tx: mpsc::Sender<MasterMessage>,
    rx: mpsc::Receiver<WorkerMessage>,
    is_working: bool,
}

/// Handles all concurrency and synchronization beween crawling jobs.
pub struct Scheduler<'a, T> {
    crawler: Crawler<'a>,
    queue: CrawlerQueue,
    callback: T,
    max_jobs: usize,
}

impl<'a, T> Scheduler<'a, T> {
    pub fn new(queue: CrawlerQueue, crawler: Crawler<'a>, max_jobs: usize, callback: T) -> Self {
        Self {
            crawler,
            queue,
            callback,
            max_jobs,
        }
    }
}

impl<T: FnMut(CrawlerMessage)> Scheduler<'_, T> {
    async fn start_master(&mut self, mut workers: Vec<WorkerHandle>) {
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
                let worker_recvs = workers.iter_mut().enumerate().filter(|(_, x)| x.is_working).map(|(i, x)| {
                    // TODO: can we remove this box pin?
                    Box::pin(async move {
                        (x.rx.recv().await, i)
                    })
                });

                let ((message, i), _, _remaining) = select_all(worker_recvs).await;
                drop(_remaining);
                let message = message.unwrap();

                workers[i].is_working = false;
                active_workers -= 1;

                match message {
                    WorkerMessage::Success(urls) => {
                        self.queue.check_and_queue_iter(urls.into_iter());
                    }
                    WorkerMessage::Failure(e, url) => {
                        (self.callback)(CrawlerMessage::Error(&url, e))
                    }
                }
            }
        }
    }

    async fn start_worker(
        crawler: Crawler<'_>,
        tx: mpsc::Sender<WorkerMessage>,
        mut rx: mpsc::Receiver<MasterMessage>,
    ) {
        loop {
            match rx.recv().await {
                None => break,
                Some(MasterMessage::NewUrl(url)) => {
                    let result = crawler.crawl_url(&url).await;

                    match result {
                        Ok(urls) => tx.send(WorkerMessage::Success(urls)).await.unwrap(),
                        Err(e) => tx.send(WorkerMessage::Failure(e, url.url)).await.unwrap(),
                    }
                }
            }
        }
    }

    pub async fn start(&mut self, roots: Vec<Url>) {
        self.queue
            .check_and_queue_iter(roots.into_iter().map(|url| CrawledUrl {
                url,
                depth: 0,
                origin_depth: 0,
            }));

        let workers = (0..self.max_jobs).map(|_| {
            let (master_tx, master_rx) = mpsc::channel::<MasterMessage>(1);
            let (worker_tx, worker_rx) = mpsc::channel::<WorkerMessage>(1);

            (
                WorkerHandle {
                    is_working: false,
                    rx: worker_rx,
                    tx: master_tx,
                },
                Self::start_worker(self.crawler.clone(), worker_tx, master_rx),
            )
        });

        let mut worker_futures = Vec::with_capacity(workers.len());
        let mut worker_channels = Vec::with_capacity(workers.len());

        for worker in workers {
            worker_futures.push(worker.1);
            worker_channels.push(worker.0);
        }

        let worker_handle = join_all(worker_futures);
        let master_handle = self.start_master(worker_channels);

        join!(worker_handle, master_handle);
    }
}
