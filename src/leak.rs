//! The [`ServiceExt::leak`](crate::ServiceExt::leak) combinator returns [`Leak`], which extends
//! the lifetime of the [`Service::Permit`].
//!
//! This can be only called on [services](Service) within an [`Arc`].
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = Arc::new(service_fn(|x| async move { x + 4 })).leak();
//! let response = svc.oneshot(3u32).await;
//! assert_eq!(7, response);
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [`Leak`] defers to the inner service.

use std::{fmt, sync::Arc};

use crate::{load::Load, Middleware, Service};

/// A wrapper [`Service`] for the [`ServiceExt::leak`](crate::ServiceExt::leak) combinator.
///
/// See the [module](crate::leak) for more information.
#[derive(Debug)]
pub struct Leak<'t, S> {
    _ref: &'t (),
    inner: Arc<S>,
}

impl<'t, S> Leak<'t, S> {
    pub(crate) fn new(inner: Arc<S>) -> Leak<'t, S> {
        Leak { _ref: &(), inner }
    }
}

/// The [`Service::Permit`] type for [`Leak`].
pub struct LeakPermit<'t, S, Request>
where
    S: Service<Request> + 't,
{
    _svc: Arc<S>,
    inner: S::Permit<'t>,
}

impl<'t, S, Request> fmt::Debug for LeakPermit<'t, S, Request>
where
    S: Service<Request> + fmt::Debug,
    for<'a> S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeakPermit")
            .field("_svc", &self._svc)
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'t, Request, S> Service<Request> for Leak<'t, S>
where
    S: Service<Request> + 't,
{
    type Response = S::Response;
    type Permit<'a> = LeakPermit<'t, S, Request>
    where
        S: 'a, 't: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        LeakPermit {
            _svc: self.inner.clone(),
            inner: unsafe { std::mem::transmute(self.inner.acquire().await) },
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit.inner, request).await
    }
}

impl<'t, S> Load for Leak<'t, S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<'t, S, T> Middleware<S> for Leak<'t, T>
where
    T: Middleware<S>,
{
    type Service = Leak<'t, T::Service>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, _ref } = self;
        let inner = Arc::into_inner(inner).expect("there cannot be inflight requests");
        Leak {
            inner: Arc::new(inner.apply(svc)),
            _ref,
        }
    }
}
