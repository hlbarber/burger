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
//! let a: fn(_) -> _ = |x: u32| ready(2 * x);
//! let b: fn(_) -> _ = |x: u32| ready(x + 3);
//! let c: fn(_) -> _ = |x: u32| ready(3 * x + 6);
//! let svc_stream = iter([service_fn(a), service_fn(b), service_fn(c)])
//!     .map(|svc| svc.pending_requests())
//!     .enumerate()
//!     .map(|(index, svc)| balance::Change::Insert(index, svc));
//! let (svc, worker) = balance::p2c(svc_stream);
//! tokio::spawn(worker);
//! let response = svc.oneshot(5u32).await;
//! # }
//! ```
//!
//! [Power of Two Random Choices]: http://www.eecs.harvard.edu/%7Emichaelm/postscripts/handbook2001.pdf
use std::{
    convert::Infallible,
    future::Future,
    hash::Hash,
    ops::{Deref, DerefMut},
    pin::pin,
    sync::Arc,
};

use futures_util::{stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use indexmap::IndexMap;
use tokio::sync::{OwnedRwLockWriteGuard, RwLock, RwLockWriteGuard};

use crate::{
    leak::{Leak, LeakPermit},
    load::Load,
    Service,
};

use super::Change;

/// Panics if empty.
#[derive(Debug)]
struct BalanceInner<S, Key> {
    services: IndexMap<Key, S>,
}

impl<S, Key> BalanceInner<S, Key>
where
    S: Load,
{
    async fn load_profile(&self) -> Vec<S::Metric> {
        self.services.values().map(|svc| svc.load()).collect()
    }
}

impl<S, Key> BalanceInner<S, Key>
where
    Key: Eq + Hash,
{
    fn insert(&mut self, key: Key, service: S) -> Option<S> {
        self.services.insert(key, service)
    }

    fn remove(&mut self, key: &Key) -> Option<S> {
        self.services.swap_remove(key)
    }

    fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    fn len(&self) -> usize {
        self.services.len()
    }
}

impl<Request, S, Key> Service<Request> for BalanceInner<S, Key>
where
    S: Service<Request> + Load,
{
    type Response = S::Response;
    type Permit<'a>
        = S::Permit<'a>
    where
        S: 'a,
        Key: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        // Race all permits.
        let mut permits: FuturesUnordered<_> = self
            .services
            .values()
            .map(|s| async move {
                let permit = s.acquire().await;
                (s, permit)
            })
            .collect();

        // Wait for first permit.
        let (first, first_permit) = permits.next().await.unwrap();

        // Try obtain second permit.
        let Some((second, second_permit)) = permits.next().now_or_never().flatten() else {
            return first_permit;
        };

        // Choose lowest load permit.
        let first_load = first.load();
        let second_load = second.load();
        if first_load < second_load {
            first_permit
        } else {
            second_permit
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}

/// A [`Service`] for the [`p2c`] constructor.
///
/// See the [module](mod@crate::balance::p2c) for more information.
#[derive(Debug)]
pub struct Balance<S, Key> {
    inner: Arc<RwLock<BalanceInner<Leak<'static, S>, Key>>>,
}

impl<S, Key> Balance<S, Key>
where
    S: Load,
{
    /// Returns [`Load::load`] for all current services.
    pub async fn load_profile(&self) -> Vec<S::Metric> {
        self.inner.read().await.load_profile().await
    }
}

impl<Request, S, Key> Service<Request> for Balance<S, Key>
where
    S: Service<Request> + Load + 'static,
    Key: Eq + Hash + 'static,
{
    type Response = S::Response;
    type Permit<'a>
        = LeakPermit<'static, S, Request>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.inner.acquire().await
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        Leak::call(permit, request).await
    }
}

enum EitherLock<'a, T> {
    Borrowed(RwLockWriteGuard<'a, T>),
    Owned(OwnedRwLockWriteGuard<T>),
}

impl<T> Deref for EitherLock<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            EitherLock::Borrowed(x) => x.deref(),
            EitherLock::Owned(x) => x.deref(),
        }
    }
}
impl<T> DerefMut for EitherLock<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            EitherLock::Borrowed(x) => x.deref_mut(),
            EitherLock::Owned(x) => x.deref_mut(),
        }
    }
}

/// The change stream has terminated.
#[derive(Debug)]
#[non_exhaustive]
pub struct Terminated;

/// Constructs a [Power of Two Random Choices] load balancer, [`Balance`] and a worker [`Future`],
/// from a [`Stream`] of [`Change`].
///
/// See [module](mod@crate::balance::p2c) for more information.
///
/// [Power of Two Random Choices]: http://www.eecs.harvard.edu/%7Emichaelm/postscripts/handbook2001.pdf
pub fn p2c<St, Key, S>(
    changes: St,
) -> (
    Balance<S, Key>,
    impl Future<Output = Result<Infallible, Terminated>>,
)
where
    St: Stream<Item = Change<Key, S>>,
    Key: Eq + Hash,
{
    let inner = Arc::new(RwLock::new(BalanceInner {
        services: IndexMap::new(),
    }));
    let balance = Balance {
        inner: inner.clone(),
    };
    // Immediately take guard so that `BalanceInner` cannot acquire when empty. Hold it until at least one service has been added.
    let empty_guard = inner.clone().try_write_owned().unwrap();
    let fut = async move {
        let mut empty_guard = Some(EitherLock::Owned(empty_guard));
        let mut changes = pin!(changes);
        while let Some(mut new_change) = changes.next().await {
            // Take the guard if not already held.
            let mut guard = if let Some(guard) = empty_guard.take() {
                guard
            } else {
                EitherLock::Borrowed(inner.write().await)
            };

            // We loop to consume as many changes while holding the lock.
            // NOTE: This will starve the `Service` if the stream is never pending.
            loop {
                // Mutate the `BalanceInner`.
                match new_change {
                    Change::Insert(key, service) => {
                        guard.insert(key, Leak::new(Arc::new(service)));
                        tracing::trace!(len = guard.len(), "inserted service");
                    }
                    Change::Remove(key) => {
                        let success = guard.remove(&key).is_some();
                        tracing::trace!(len = guard.len(), success, "removed service")
                    }
                };

                match changes.next().now_or_never() {
                    // Stream yielded.
                    Some(Some(change)) => {
                        new_change = change;
                    }
                    // Stream terminated.
                    Some(None) => {
                        return Err(Terminated);
                    }
                    // Stream pending.
                    None => {
                        // Retain the guard if `BalanceInner` is empty.
                        if guard.is_empty() {
                            empty_guard = Some(guard);
                        } else {
                            let _guard = empty_guard.take();
                            drop(_guard);
                        }
                        break;
                    }
                }
            }
        }
        Err(Terminated)
    };

    (balance, fut)
}
