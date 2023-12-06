use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use async_lock::{Semaphore, SemaphoreGuard};
use futures_util::future::MaybeDone;
use pin_project_lite::pin_project;

use crate::Service;

pub struct ConcurrencyLimit<S> {
    pub(crate) semaphore: Semaphore,
    pub(crate) inner: S,
}

pub struct ConcurrencyLimitPermit<'a, Inner> {
    inner: Inner,
    permit: SemaphoreGuard<'a>,
}

pin_project! {
    pub struct ConcurencyLimitAcquire<'a, Inner>
    where
        Inner: Future
    {
        #[pin]
        inner: MaybeDone<Inner>,
        #[pin]
        acquire: MaybeDone<async_lock::futures::Acquire<'a>>
    }
}

pin_project! {
    pub struct ConcurrencyLimitFuture<'a, Inner> {
        #[pin]
        inner: Inner,
        _permit: SemaphoreGuard<'a>,
    }
}

impl<'a, Inner> Future for ConcurrencyLimitFuture<'a, Inner>
where
    Inner: Future,
{
    type Output = Inner::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

impl<'a, Inner> Future for ConcurencyLimitAcquire<'a, Inner>
where
    Inner: Future,
{
    type Output = ConcurrencyLimitPermit<'a, Inner::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut().project();
        ready!(this.inner.as_mut().poll(cx));
        ready!(this.acquire.as_mut().poll(cx));
        Poll::Ready(ConcurrencyLimitPermit {
            inner: this
                .inner
                .as_mut()
                .take_output()
                .expect("futures cannot be polled after completion"),
            permit: this
                .acquire
                .as_mut()
                .take_output()
                .expect("futures cannot be polled after completion"),
        })
    }
}

impl<Request, S> Service<Request> for ConcurrencyLimit<S>
where
    S: Service<Request>,
{
    type Future<'a> = ConcurrencyLimitFuture<'a, S::Future<'a>> where Self: 'a;

    type Permit<'a> = ConcurrencyLimitPermit<'a, S::Permit<'a>>
    where
        S: 'a;

    type Acquire<'a> = ConcurencyLimitAcquire<'a, S::Acquire<'a>>
    where
        S: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        ConcurencyLimitAcquire {
            inner: MaybeDone::Future(self.inner.acquire()),
            acquire: MaybeDone::Future(self.semaphore.acquire()),
        }
    }

    fn call<'a>(guard: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        ConcurrencyLimitFuture {
            inner: S::call(guard.inner, request),
            _permit: guard.permit,
        }
    }
}
