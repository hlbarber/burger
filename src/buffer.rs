use std::fmt;

use futures_util::FutureExt;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{Service, ServiceExt};

#[derive(Debug)]
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

impl<'a, S, Request> fmt::Debug for BufferPermit<'a, S, Request>
where
    S: Service<Request>,
    BufferPermitInner<'a, S, Request>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferPermit")
            .field("inner", &self.inner)
            .finish()
    }
}

enum BufferPermitInner<'a, S, Request>
where
    S: Service<Request>,
{
    Eager(S::Permit<'a>),
    Buffered(&'a S, SemaphorePermit<'a>),
}

impl<'a, S, Request> fmt::Debug for BufferPermitInner<'a, S, Request>
where
    S: Service<Request> + fmt::Debug,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eager(arg0) => f.debug_tuple("Eager").field(arg0).finish(),
            Self::Buffered(arg0, arg1) => {
                f.debug_tuple("Buffered").field(arg0).field(arg1).finish()
            }
        }
    }
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
