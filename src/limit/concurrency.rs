use tokio::sync::{Semaphore, SemaphorePermit};

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
    _semaphore_permit: SemaphorePermit<'a>,
}

impl<Request, S> Service<Request> for ConcurrencyLimit<S>
where
    S: Service<Request>,
{
    type Response<'a> = S::Response<'a>;
    type Permit<'a> = ConcurrencyLimitPermit<'a, S::Permit<'a>>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        ConcurrencyLimitPermit {
            inner: self.inner.acquire().await,
            _semaphore_permit: self.semaphore.acquire().await.expect("not closed"),
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response<'a> {
        S::call(permit.inner, request).await
    }
}
