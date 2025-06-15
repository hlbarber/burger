//! [`tower`] is an established service abstraction.
//!
//! A [`tower::Service`] is converted into a [`burger::Service`](crate::Service) using the
//! [`compat`] function. We require [`Clone`] in order to convert from [`tower`]s
//! `&mut self` signature to [`burger`]s `&self` signature.
//!
//! Note that [`tower`], in general, has no disarm mechanism. This means that
//! dropping the permit is _not_ sufficient to restore the service to a reasonable state.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = tower::service_fn(|x| async move { Ok::<_, ()>(32) });
//! let svc = compat(svc);
//! let response = svc.oneshot(()).await;
//! assert_eq!(response, Ok(32));
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [`Compat`] implementation uses [`tower::load::Load`].

use tower::{load::Load, ServiceExt as _};

use crate::Service;

/// A compatibility wrapper for [`tower::Service`].
///
/// See [module](mod@crate::compat) for more information.
#[derive(Debug)]
pub struct Compat<S> {
    inner: S,
}

impl<Request, S> Service<Request> for Compat<S>
where
    S: tower::Service<Request> + Clone,
{
    type Response = Result<S::Response, S::Error>;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let svc = self.inner.clone().ready_oneshot().await;
        async move |request| svc?.call(request).await
    }
}

/// Converts a [`tower::Service`] to a [`burger::Service`](Service).
///
/// See the [module](mod@crate::compat) for more information.
pub fn compat<S>(inner: S) -> Compat<S> {
    Compat { inner }
}

impl<S> Load for Compat<S>
where
    S: tower::load::Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
