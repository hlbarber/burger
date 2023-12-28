//! Load is a measurement of the amount of work a service is experiencing. The [Load] trait
//! provides an interface to measure it and therefore informs business logic in applications such
//! as load balancers.

use std::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::Service;

/// A measurement of load on a [Service].
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

/// The [Service::Permit] type for [PendingRequests].
pub struct PendingRequestsPermit<'a, S, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    count: &'a AtomicUsize,
}

impl<'a, S, Request> fmt::Debug for PendingRequestsPermit<'a, S, Request>
where
    S: Service<Request> + 'a,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingRequestsPermit")
            .field("inner", &self.inner)
            .field("count", &self.count)
            .finish()
    }
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
    type Permit<'a> = PendingRequestsPermit<'a, S, Request>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        PendingRequestsPermit {
            inner: self.inner.acquire().await,
            count: &self.count,
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        permit.count.fetch_add(1, Ordering::Release);
        let response = S::call(permit.inner, request).await;
        permit.count.fetch_sub(1, Ordering::Release);
        response
    }
}

impl<S> Load for PendingRequests<S> {
    type Metric = usize;

    fn load(&self) -> Self::Metric {
        self.count.load(Ordering::Acquire)
    }
}
