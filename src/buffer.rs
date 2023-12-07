use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use async_lock::{Semaphore, SemaphoreGuard};
use futures_util::future::MaybeDone;
use pin_project_lite::pin_project;

use crate::{oneshot::Oneshot, Service, ServiceExt};

pub struct Buffer<S> {
    inner: S,
    semaphore: Semaphore,
}

impl<S> Buffer<S> {
    pub(crate) fn new(inner: S, capacity: usize) -> Self {
        Self {
            inner,
            semaphore: Semaphore::new(capacity),
        }
    }
}

pub struct BufferPermit<'a, S> {
    inner: &'a S,
    _semaphore_permit: SemaphoreGuard<'a>,
}

pin_project! {
    pub struct BufferAcquire<'a, S>
    {
        inner: Option<&'a S>,
        #[pin]
        semaphore_acquire: MaybeDone<async_lock::futures::Acquire<'a>>
    }
}

impl<'a, S> Future for BufferAcquire<'a, S> {
    type Output = BufferPermit<'a, S>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut().project();
        ready!(this.semaphore_acquire.as_mut().poll(cx));
        Poll::Ready(BufferPermit {
            inner: this
                .inner
                .take()
                .expect("futures cannot be polled after completion"),
            _semaphore_permit: this
                .semaphore_acquire
                .as_mut()
                .take_output()
                .expect("futures cannot be polled after completion"),
        })
    }
}

impl<Request, S> Service<Request> for Buffer<S>
where
    S: Service<Request>,
{
    type Future<'a> = Oneshot<'a, S, Request> where S: 'a;
    type Permit<'a> = BufferPermit<'a, S>
    where
        S: 'a;
    type Acquire<'a> = BufferAcquire<'a, S>
    where
        S: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        BufferAcquire {
            inner: Some(&self.inner),
            semaphore_acquire: MaybeDone::Future(self.semaphore.acquire()),
        }
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        permit.inner.oneshot(request)
    }
}
