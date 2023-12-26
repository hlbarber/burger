//! Load is a measurement of work a service is experiencing. The [Load] trait provides an
//! interface to measure it and therefore can drive business logic in applications such as load
//! balancers.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::Service;

/// Measurements of load on a [Service].
pub trait Load {
    /// The metric type outputted by [load](Load::load).
    type Metric: PartialOrd;

    /// Measures the current load.
    fn load(&self) -> Self::Metric;
}

/// A wrapper [Service] providing a [Load] implementation based on the number of pending requests.
///
/// TODO: Make it so.
#[derive(Debug)]
pub struct PendingRequests<S> {
    inner: S,
    count: AtomicUsize,
}

impl<S> PendingRequests<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            count: AtomicUsize::new(0),
        }
    }
}

impl<Request, S> Service<Request> for PendingRequests<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.count.fetch_add(1, Ordering::Release);
        let permit = self.inner.acquire().await;
        self.count.fetch_sub(1, Ordering::Release);
        permit
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}

impl<S> Load for PendingRequests<S> {
    type Metric = usize;

    fn load(&self) -> Self::Metric {
        self.count.load(Ordering::Acquire)
    }
}
