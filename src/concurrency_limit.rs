use std::fmt;

use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{balance::Load, Service};

#[derive(Debug)]
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

pub struct ConcurrencyLimitPermit<'a, S, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    _semaphore_permit: SemaphorePermit<'a>,
}

impl<'a, S, Request> fmt::Debug for ConcurrencyLimitPermit<'a, S, Request>
where
    S: Service<Request>,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcurrencyLimitPermit")
            .field("inner", &self.inner)
            .field("_semaphore_permit", &self._semaphore_permit)
            .finish()
    }
}

impl<Request, S> Service<Request> for ConcurrencyLimit<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = ConcurrencyLimitPermit<'a, S, Request>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        ConcurrencyLimitPermit {
            inner: self.inner.acquire().await,
            _semaphore_permit: self.semaphore.acquire().await.expect("not closed"),
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        S::call(permit.inner, request).await
    }
}

impl<S> Load for ConcurrencyLimit<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
