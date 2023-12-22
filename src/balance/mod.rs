use std::{
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

use futures_util::Stream;

use crate::Service;

pub mod p2c;

pub trait Load {
    type Metric: PartialOrd;

    async fn load(&self) -> Self::Metric;
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

    async fn load(&self) -> Self::Metric {
        self.count.load(Ordering::Acquire)
    }
}

pub enum Change<K, V> {
    Insert(K, V),
    Remove(K),
}

pin_project_lite::pin_project! {
    pub struct ChangeMapService<St, F> {
        #[pin]
        stream: St,
        f: F,
    }
}

impl<St, F, Key, S, Output> Stream for ChangeMapService<St, F>
where
    St: Stream<Item = Change<Key, S>>,
    F: Fn(S) -> Output,
{
    type Item = Change<Key, Output>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.stream.poll_next(cx).map(|opt| {
            opt.map(|next| match next {
                Change::Insert(key, inner) => Change::Insert(key, (this.f)(inner)),
                Change::Remove(key) => Change::Remove(key),
            })
        })
    }
}

pub trait DiscoverExt<Key, S>: Stream<Item = Change<Key, S>> {
    fn change_map_service<F>(self, f: F) -> ChangeMapService<Self, F>
    where
        Self: Sized,
    {
        ChangeMapService { stream: self, f }
    }
}

impl<Key, S, St> DiscoverExt<Key, S> for St where St: Stream<Item = Change<Key, S>> {}
