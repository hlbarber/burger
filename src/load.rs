//! Load is a measurement of the amount of work a service is experiencing. The [`Load`] trait
//! provides an interface to measure it and therefore informs business logic in applications such
//! as load balancers.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{Middleware, Service};

/// A measurement of load on a [`Service`].
pub trait Load {
    /// The metric type outputted by [`Load`](Load::load).
    type Metric: PartialOrd;

    /// Measures the current load.
    fn load(&self) -> Self::Metric;
}

/// A wrapper [`Service`] providing a [`Load`] implementation based on the number of pending requests.
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

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let permit = self.inner.acquire().await;
        async |request| {
            self.count.fetch_add(1, Ordering::Release);
            let response = permit(request).await;
            self.count.fetch_sub(1, Ordering::Release);
            response
        }
    }
}

impl<S> Load for PendingRequests<S> {
    type Metric = usize;

    fn load(&self) -> Self::Metric {
        self.count.load(Ordering::Acquire)
    }
}

impl<S, T> Middleware<S> for PendingRequests<T>
where
    T: Middleware<S>,
{
    type Service = PendingRequests<T::Service>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, count } = self;
        PendingRequests {
            inner: inner.apply(svc),
            count,
        }
    }
}
