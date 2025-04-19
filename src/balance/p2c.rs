//! The [`p2c`] function returns [`Balance`], which implements the
//! [Power of Two Random Choices] load balancing algorithm, The implementation acquires two
//! permits and then chooses the lowest [`Load`] of the two.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use futures::stream::{iter, StreamExt};
//! # use std::future::ready;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = balance::p2c(svc_stream);
//! svc.handle().add_service(0, service_fn(|x: u32| ready(2 * x)).pending_requests());
//! let response = svc.oneshot(5u32).await;
//! # }
//! ```
//!
//! [Power of Two Random Choices]: http://www.eecs.harvard.edu/%7Emichaelm/postscripts/handbook2001.pdf
use std::{hash::Hash, sync::Arc};

use futures_util::{stream::FuturesUnordered, FutureExt, StreamExt};
use indexmap::IndexMap;
use tokio::sync::watch;

use crate::{load::Load, Service, ServiceExt};

#[derive(Debug)]
pub struct P2cHandle<S, Key> {
    sender: watch::Sender<IndexMap<Key, Arc<S>>>,
}

impl<S, Key> P2cHandle<S, Key>
where
    Key: Eq + Hash,
{
    pub fn add_service(&self, key: Key, svc: Arc<S>) -> &Self {
        self.sender.send_modify(|map| {
            map.insert(key, svc);
        });
        self
    }

    pub fn remove_service(&self, key: &Key) -> &Self {
        self.sender.send_modify(|map| {
            map.swap_remove(key);
        });
        self
    }
}

/// A [`Service`] for the [`p2c`] constructor.
///
/// See the [module](mod@crate::balance::p2c) for more information.
#[derive(Debug)]
pub struct Balance<S, Key> {
    services: watch::Receiver<IndexMap<Key, Arc<S>>>,
    sender: watch::Sender<IndexMap<Key, Arc<S>>>,
}

impl<S, Key> Balance<S, Key> {
    pub fn handle(&self) -> P2cHandle<S, Key> {
        P2cHandle {
            sender: self.sender.clone(),
        }
    }
}

impl<S, Key> Balance<S, Key>
where
    S: Load,
{
    pub async fn load_profile(&self) -> Vec<S::Metric> {
        self.services
            .borrow()
            .values()
            .map(|svc| svc.load())
            .collect()
    }
}

impl<Request, S, Key> Service<Request> for Balance<S, Key>
where
    S: Service<Request> + Load,
    Request: 'static,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let mut services = self.services.clone();

        // Wait until at least one service.
        let services: Vec<_> = loop {
            let index_map = services.borrow_and_update();
            if index_map.is_empty() {
                drop(index_map);
                services.changed().await.expect("sender is alive");
                continue;
            }
            break index_map.values().cloned().collect();
        };

        // Race all permits.
        let mut permits: FuturesUnordered<_> = services
            .into_iter()
            .map(|s| async move {
                let load = s.load();
                let permit = s.acquire_owned().await;
                (load, permit)
            })
            .collect();

        // Wait for first permit.
        let (first_load, first_permit) = permits.next().await.unwrap();

        // Try obtain second permit.
        let Some((second_load, second_permit)) = permits.next().now_or_never().flatten() else {
            return first_permit;
        };

        // Choose lowest load permit.
        if first_load < second_load {
            first_permit
        } else {
            second_permit
        }
    }
}

/// Constructs a [Power of Two Random Choices] load balancer, [`Balance`] and a worker [`Future`],
/// from a [`Stream`] of [`Change`].
///
/// See [module](mod@crate::balance::p2c) for more information.
///
/// [Power of Two Random Choices]: http://www.eecs.harvard.edu/%7Emichaelm/postscripts/handbook2001.pdf
pub fn p2c<Key, S>() -> Balance<S, Key> {
    let (sender, services) = watch::channel(IndexMap::default());
    Balance { services, sender }
}
