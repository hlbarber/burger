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

pub struct BufferPermit<'a, S> {
    inner: &'a S,
    _semaphore_permit: SemaphorePermit<'a>,
}

impl<Request, S> Service<Request> for Buffer<S>
where
    S: Service<Request>,
{
    type Response<'a> = S::Response<'a>;
    type Permit<'a> = BufferPermit<'a, S>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        BufferPermit {
            inner: &self.inner,
            _semaphore_permit: self.semaphore.acquire().await.expect("not closed"),
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response<'_> {
        permit.inner.oneshot(request).await
    }
}
