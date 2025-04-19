//! Given a collection of [services](Service) and a [`Picker`], the [`steer`] function constructs a
//! [`Steer`] [`Service`].
//!
//! The [`Service::acquire`] on [`Steer`] acquires _all_ [permits](Service::Permit) from the
//! collection, the [`Picker`] then selects which permit and [`Service`] to [`Service::call`].
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//!
//! # #[tokio::main]
//! async fn main() {
//! struct AlwaysFirst;
//!
//! impl<S, Request> steer::Picker<S, Request> for AlwaysFirst {
//!     fn pick(&self, services: &[S], _request: &Request) -> usize {
//!         0
//!     }
//! }
//!
//! let svcs = (0..10).map(|index| service_fn(move |x| async move { index * x }));
//! let picker = AlwaysFirst;
//! let svc = steer(svcs, picker);
//! let response = svc.oneshot(7).await;
//! assert_eq!(0, response);
//! # }
//! ```
//!
//! # Load
//!
//! This has _no_ [`Load`](crate::load::Load) implementation.

use futures_util::future::join_all;

use crate::Service;

/// A wrapper [`Service`] for the [`steer`] constructor.
///
/// See the [module](mod@crate::steer) for more information.
#[derive(Debug)]
pub struct Steer<S, P> {
    services: Box<[S]>,
    picker: P,
}

/// Picks a service from an underlying collection of services to steer requests.
pub trait Picker<S, Request> {
    /// Returns the index of the picked service.
    ///
    /// The returned index MUST be valid.
    fn pick(&self, services: &[S], request: &Request) -> usize;
}

impl<Request, S, P> Service<Request> for Steer<S, P>
where
    S: Service<Request>,
    P: Picker<S, Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let mut permits = join_all(self.services.iter().map(|x| x.acquire())).await;
        async |request| {
            let index = self.picker.pick(&self.services, &request);
            let permit = permits.swap_remove(index);
            drop(permits);
            permit(request).await
        }
    }
}

/// Constructs a [`Service`] from a [collection](IntoIterator) of services whose [`Service::call`] is
/// steered via a [`Picker`].
///
/// See [module](mod@crate::steer) for more information.
pub fn steer<S, P>(services: impl IntoIterator<Item = S>, picker: P) -> Steer<S, P> {
    Steer {
        services: services.into_iter().collect(),
        picker,
    }
}
