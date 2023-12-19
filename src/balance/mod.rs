use std::sync::atomic::{AtomicUsize, Ordering};

use futures_util::{stream::Map, Stream, StreamExt};

use crate::Service;

pub mod p2c;

pub trait Load {
    type Metric: PartialOrd;

    async fn load(&self) -> Self::Metric;
}

pub struct PendingRequests<S> {
    inner: S,
    count: AtomicUsize,
}

impl<Request, S> Service<Request> for PendingRequests<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.count.fetch_add(1, Ordering::Release);
        let permit = self.inner.acquire().await;
        self.count.fetch_sub(1, Ordering::Release);
        permit
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}

impl<S> Load for PendingRequests<S> {
    type Metric = usize;

    async fn load(&self) -> Self::Metric {
        self.count.load(Ordering::Acquire)
    }
}

pub enum Change<K, V> {
    Insert(K, V),
    Remove(K),
}

pub trait DiscoverExt<Key, S>: Stream<Item = Change<Key, S>> {
    fn pending_requests(self) -> Map<Self, fn(Change<Key, S>) -> Change<Key, PendingRequests<S>>>
    where
        Self: Sized,
    {
        self.map(|change| match change {
            Change::Insert(key, inner) => Change::Insert(
                key,
                PendingRequests {
                    inner,
                    count: AtomicUsize::default(),
                },
            ),
            Change::Remove(key) => Change::Remove(key),
        })
    }
}

impl<Key, S, St> DiscoverExt<Key, S> for St where St: Stream<Item = Change<Key, S>> {}
