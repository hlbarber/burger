use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_util::{future::Map as MapFuture, FutureExt};

use crate::Service;

pub struct Map<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Map<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

pub struct MapPermit<'a, Inner, F> {
    inner: Inner,
    closure: &'a F,
}

pin_project_lite::pin_project! {
    pub struct MapAcquire<'a, Inner, F> {
        #[pin]
        inner: Inner,
        closure: &'a F
    }
}

impl<'a, Inner, F> Future for MapAcquire<'a, Inner, F>
where
    Inner: Future,
{
    type Output = MapPermit<'a, Inner::Output, F>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let inner = ready!(this.inner.poll(cx));
        Poll::Ready(MapPermit {
            inner,
            closure: this.closure,
        })
    }
}

impl<Request, S, F, Output> Service<Request> for Map<S, F>
where
    S: Service<Request>,
    for<'a> F: Fn(<S::Future<'a> as Future>::Output) -> Output,
{
    type Future<'a> = MapFuture<S::Future<'a>, &'a F> where S: 'a, F: 'a;
    type Permit<'a> = MapPermit<'a, S::Permit<'a>, F> where S: 'a, F: 'a;
    type Acquire<'a> = MapAcquire<'a, S::Acquire<'a>, F> where F: 'a, S: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        MapAcquire {
            inner: self.inner.acquire(),
            closure: &self.closure,
        }
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        S::call(permit.inner, request).map(permit.closure)
    }
}
