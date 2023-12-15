use futures_util::FutureExt;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{Service, ServiceExt};

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

pub struct BufferPermit<'a, S, Request>
where
    S: Service<Request>,
{
    inner: BufferPermitInner<'a, S, Request>,
}

enum BufferPermitInner<'a, S, Request>
where
    S: Service<Request>,
{
    Eager(S::Permit<'a>),
    Buffered(&'a S, SemaphorePermit<'a>),
}

impl<Request, S> Service<Request> for Buffer<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = BufferPermit<'a, S, Request>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        BufferPermit {
            inner: match self.inner.acquire().now_or_never() {
                Some(some) => BufferPermitInner::Eager(some),
                None => BufferPermitInner::Buffered(
                    &self.inner,
                    self.semaphore.acquire().await.expect("not closed"),
                ),
            },
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        match permit.inner {
            BufferPermitInner::Eager(permit) => S::call(permit, request).await,
            BufferPermitInner::Buffered(service, _permit) => service.oneshot(request).await,
        }
    }
}
