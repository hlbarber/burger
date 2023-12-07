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
    inner: S,
    semaphore: Semaphore,
}

impl<S> ConcurrencyLimit<S> {
    pub(crate) fn new(inner: S, n_permits: usize) -> Self {
        Self {
            inner,
            semaphore: Semaphore::new(n_permits),
        }
    }
}

pub struct ConcurrencyLimitPermit<'a, Inner> {
    inner: Inner,
    semaphore_permit: SemaphoreGuard<'a>,
}

pin_project! {
    pub struct ConcurencyLimitAcquire<'a, Inner>
    where
        Inner: Future
    {
        #[pin]
        inner: MaybeDone<Inner>,
        #[pin]
        semaphore_acquire: MaybeDone<async_lock::futures::Acquire<'a>>
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
        ready!(this.semaphore_acquire.as_mut().poll(cx));
        Poll::Ready(ConcurrencyLimitPermit {
            inner: this
                .inner
                .as_mut()
                .take_output()
                .expect("futures cannot be polled after completion"),
            semaphore_permit: this
                .semaphore_acquire
                .as_mut()
                .take_output()
                .expect("futures cannot be polled after completion"),
        })
    }
}

pin_project! {
    pub struct ConcurrencyLimitFuture<'a, Inner> {
        #[pin]
        inner: Inner,
        _semaphore_permit: SemaphoreGuard<'a>,
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

impl<Request, S> Service<Request> for ConcurrencyLimit<S>
where
    S: Service<Request>,
{
    type Future<'a> = ConcurrencyLimitFuture<'a, S::Future<'a>> where S: 'a;
    type Permit<'a> = ConcurrencyLimitPermit<'a, S::Permit<'a>>
    where
        S: 'a;
    type Acquire<'a> = ConcurencyLimitAcquire<'a, S::Acquire<'a>>
    where
        S: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        ConcurencyLimitAcquire {
            inner: MaybeDone::Future(self.inner.acquire()),
            semaphore_acquire: MaybeDone::Future(self.semaphore.acquire()),
        }
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        ConcurrencyLimitFuture {
            inner: S::call(permit.inner, request),
            _semaphore_permit: permit.semaphore_permit,
        }
    }
}
