//! [tower] is an established service abstraction.
//!
//! Convert between [tower::Service] and [burger::Service](crate::Service) using the [`compat`]
//! function.
//!
//! Note that [tower], in general, has no disarm mechanism. This means that
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
//! The [Load::load] on [Compat] implementation uses [tower::load::Load].

use std::{
    future::poll_fn,
    sync::{Mutex, MutexGuard},
};

use tower::{load::Load, Service as TowerService};

use crate::Service;

/// A compatibility wrapper for [tower::Service].
#[derive(Debug)]
pub struct Compat<S> {
    inner: Mutex<S>,
}

impl<Request, S> Service<Request> for Compat<S>
where
    S: TowerService<Request>,
{
    type Response = Result<S::Response, S::Error>;
    type Permit<'a> = Result<MutexGuard<'a, S>, S::Error>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        poll_fn(|cx| self.inner.lock().unwrap().poll_ready(cx))
            .await
            .map(|_| self.inner.lock().unwrap())
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        // https://github.com/rust-lang/rust-clippy/issues/6446
        let fut = {
            let mut guard = permit?;
            guard.call(request)
        };
        fut.await
    }
}

/// Converts a [tower::Service] to a [burger::Service](Service).
///
/// See the [module](mod@crate::compat) for more details.
pub fn compat<S>(inner: S) -> Compat<S> {
    Compat {
        inner: Mutex::new(inner),
    }
}

impl<S> Load for Compat<S>
where
    S: tower::load::Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.lock().unwrap().load()
    }
}
