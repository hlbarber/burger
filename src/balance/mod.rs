use std::sync::atomic::{AtomicUsize, Ordering};

use crate::Service;

pub mod p2c;

pub trait Load {
    type Metric: PartialOrd;

    fn load(&self) -> Self::Metric;
}

#[derive(Debug)]
pub struct PendingRequests<S> {
    inner: S,
    count: AtomicUsize,
}

impl<S> PendingRequests<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            count: AtomicUsize::new(0),
        }
    }
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

    fn load(&self) -> Self::Metric {
        self.count.load(Ordering::Acquire)
    }
}

pub enum Change<K, V> {
    Insert(K, V),
    Remove(K),
}
