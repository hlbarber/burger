use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_util::{future::Then as ThenFuture, FutureExt};

use crate::Service;

pub struct Then<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Then<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

pub struct ThenPermit<'a, Inner, F> {
    inner: Inner,
    closure: &'a F,
}

pin_project_lite::pin_project! {
    pub struct ThenAcquire<'a, Inner, F> {
        #[pin]
        inner: Inner,
        closure: &'a F
    }
}

impl<'a, Inner, F> Future for ThenAcquire<'a, Inner, F>
where
    Inner: Future,
{
    type Output = ThenPermit<'a, Inner::Output, F>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let inner = ready!(this.inner.poll(cx));
        Poll::Ready(ThenPermit {
            inner,
            closure: this.closure,
        })
    }
}

impl<Request, S, F, Fut2> Service<Request> for Then<S, F>
where
    S: Service<Request>,
    for<'a> F: Fn(<S::Future<'a> as Future>::Output) -> Fut2,
    Fut2: Future,
{
    type Future<'a> = ThenFuture<S::Future<'a>, Fut2, &'a F> where S: 'a, F: 'a;
    type Permit<'a> = ThenPermit<'a, S::Permit<'a>, F> where S: 'a, F: 'a;
    type Acquire<'a> = ThenAcquire<'a, S::Acquire<'a>, F> where F: 'a, S: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        ThenAcquire {
            inner: self.inner.acquire(),
            closure: &self.closure,
        }
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        S::call(permit.inner, request).then(permit.closure)
    }
}
