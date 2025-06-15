#![allow(async_fn_in_trait)]
#![deny(missing_docs, missing_debug_implementations)]

//! An experimental service framework.
//!
//! The [`Service`] trait is the central abstraction. It is an
//! [asynchronous function](Service::call), accepting a request and returning a response, which
//! can only be executed _after_ a [permit](Service::Permit) is [acquired](Service::acquire).
//!
//! The root exports [`Service`] constructors, and an extension trait, [`ServiceExt`] containing
//! combinators to modify a [`Service`]. Both the combinators and constructors each have an
//! associated module containing related documentation, traits, and types.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use tokio::time::sleep;
//! # use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x| async move {
//!     sleep(Duration::from_secs(1)).await;
//!     2 * x
//! })
//! .map(|x| x + 3)
//! .concurrency_limit(1)
//! .buffer(3)
//! .load_shed();
//! let response = svc.oneshot(30).await;
//! assert_eq!(Ok(63), response);
//! # }
//! ```
//!
//! # Usage
//!
//! A typical [`Service`] will consist of distinct layers, each providing specific dynamics. The
//! following flowchart attempts to categorize the exports of this crate:
//!
//! <pre class="mermaid" style="text-align:center">
#![doc = include_str!("flowchart.mmd")]
//! </pre>
//! <script type="module">
//! import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs';
//! let config = { theme: "dark", startOnLoad: true, flowchart: { useMaxWidth: true, htmlLabels: true } };
//! mermaid.initialize(config);
//! </script>

pub mod balance;
pub mod buffer;
#[cfg(feature = "compat")]
pub mod compat;
pub mod concurrency_limit;
pub mod depressurize;
pub mod either;
pub mod load;
pub mod load_shed;
pub mod map;
pub mod rate_limit;
pub mod retry;
pub mod select;
pub mod service_fn;
pub mod steer;
pub mod then;

use std::{convert::Infallible, sync::Arc, time::Duration};

use buffer::Buffer;
use concurrency_limit::ConcurrencyLimit;
use depressurize::Depressurize;
use either::Either;
use load::{Load, PendingRequests};
use load_shed::LoadShed;
use map::Map;
use rate_limit::RateLimit;
use retry::Retry;
use then::Then;

#[cfg(feature = "compat")]
#[doc(inline)]
pub use compat::compat;
#[doc(inline)]
pub use select::select;
#[doc(inline)]
pub use service_fn::service_fn;
#[doc(inline)]
pub use steer::steer;

/// An asynchronous function call, which can only be executed _after_ obtaining a permit.
///
/// # Example
///
/// ```rust
/// use burger::{service_fn::ServiceFn, *};
///
/// # #[tokio::main]
/// # async fn main() {
/// let svc = service_fn(|x: usize| async move { x.to_string() });
/// let permit = svc.acquire().await;
/// let response = ServiceFn::call(permit, 32).await;
/// # }
/// ```
pub trait Service<Request> {
    /// The type produced by the service call.
    type Response;

    /// Obtains a permit.
    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response;
}

/// An extension trait for [`Service`].
pub trait ServiceExt<Request>: Service<Request> {
    /// Returns the [`Service`] wrapped in an [`Arc`].
    fn arc(self) -> Arc<Self>
    where
        Self: Sized,
    {
        Arc::new(self)
    }

    /// Returns a permit with an extended lifetime.
    async fn acquire_owned<'a>(
        self: Arc<Self>,
    ) -> impl AsyncFnOnce(Request) -> Self::Response + use<'a, Request, Self>
    where
        Request: 'a,
        Self: 'a,
    {
        let this = Arc::clone(&self);
        let this_ref: &Self = this.as_ref();
        let this_ref: &'a Self = unsafe { std::mem::transmute(this_ref) };
        let permit = this_ref.acquire().await;

        async move |request| {
            let response = permit(request).await;
            drop(this);
            response
        }
    }

    /// Acquires the [`Service::Permit`] and then immediately uses it to [call](Service::call) the
    /// [`Service`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use burger::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let svc = service_fn(|x: usize| async move { x.to_string() });
    /// let response = svc.oneshot(32).await;
    /// # }
    /// ```
    async fn oneshot(&self, request: Request) -> Self::Response
    where
        Self: Sized,
    {
        self.acquire().await(request).await
    }

    /// Extends the service using a closure accepting [`Self::Response`](Service::Response) and
    /// returning a [`Future`](std::future::Future).
    ///
    /// See the [module](then) for more information.
    fn then<F>(self, closure: F) -> Then<Self, F>
    where
        Self: Sized,
    {
        Then::new(self, closure)
    }

    /// Extends the service using a closure accepting [Self::Response](Service::Response) and returning a
    /// [`Future`](std::future::Future).
    ///
    /// See the [module](map) for more information.
    fn map<F>(self, closure: F) -> Map<Self, F>
    where
        Self: Sized,
    {
        Map::new(self, closure)
    }

    /// Applies a concurrency limit to the service with a specified number of permits.
    ///
    /// See [concurrency limit](concurrency_limit) module for more information.
    fn concurrency_limit(self, n_permits: usize) -> ConcurrencyLimit<Self>
    where
        Self: Sized,
    {
        ConcurrencyLimit::new(self, n_permits)
    }

    /// Applies load shedding to the service.
    ///
    /// See [module](load_shed) for more information.
    fn load_shed(self) -> LoadShed<Self>
    where
        Self: Sized,
    {
        LoadShed::new(self)
    }

    /// Applies buffering to the service with a specified capacity.
    ///
    /// See the [module](buffer) for more information.
    fn buffer(self, capacity: usize) -> Buffer<Self>
    where
        Self: Sized,
    {
        Buffer::new(self, capacity)
    }

    /// Applies rate limiting to the service with a specified interval and number of permits.
    ///
    /// See the [module](rate_limit) for more information.
    fn rate_limit(self, interval: Duration, permits: usize) -> RateLimit<Self>
    where
        Self: Sized,
    {
        RateLimit::new(self, interval, permits)
    }

    /// Applies retries to tbe service with a specified [Policy](crate::retry::Policy).
    ///
    /// See the [module](retry) for more information.
    fn retry<P>(self, policy: P) -> Retry<Self, P>
    where
        Self: Sized,
    {
        Retry::new(self, policy)
    }

    /// Depressurizes the service.
    ///
    /// See the [module](depressurize) for more information,
    fn depressurize(self) -> Depressurize<Self>
    where
        Self: Sized,
    {
        Depressurize::new(self)
    }

    /// Records [`Load`] on the service, measured by number of pending requests.
    ///
    /// See the [load] module for more information.
    fn pending_requests(self) -> PendingRequests<Self>
    where
        Self: Sized,
    {
        PendingRequests::new(self)
    }

    /// Wraps as [Either::Left]. For the other variant see [ServiceExt::right].
    ///
    /// See the [module](either) for more information.
    fn left<T>(self) -> Either<Self, T>
    where
        Self: Sized,
    {
        Either::Left(self)
    }

    /// Wraps as [Either::Right]. For the other variant see [ServiceExt::right].
    ///
    /// See the [module](either) for more information.
    fn right<T>(self) -> Either<T, Self>
    where
        Self: Sized,
    {
        Either::Right(self)
    }
}

impl<Request, S> ServiceExt<Request> for S where S: Service<Request> {}

/// A fallible [`Service`].
pub trait TryService<Request>: Service<Request, Response = Result<Self::Ok, Self::Error>> {
    /// The [`Result::Ok`] variant of the [`Service::Response`].
    type Ok;
    /// The [`Result::Err`] variant of the [`Service::Response`].
    type Error;
}

impl<Request, Ok, Error, S> TryService<Request> for S
where
    S: Service<Request, Response = Result<Ok, Error>>,
{
    type Ok = Ok;
    type Error = Error;
}

impl<Request, S> Service<Request> for Arc<S>
where
    S: Service<Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        S::acquire(self).await
    }
}

impl<S> Load for Arc<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        S::load(self)
    }
}

impl<'t, Request, S> Service<Request> for &'t S
where
    S: Service<Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        S::acquire(self).await
    }
}

impl<S> Load for &S
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        S::load(self)
    }
}

/// A middleware, used to incrementally add behaviour to a [`Service`].
pub trait Middleware<S> {
    /// The resultant service.
    type Service;

    /// Applies this middleware to an existing service.
    fn apply(self, svc: S) -> Self::Service;
}

/// The root of a chain of [`Middleware`]s.
///
/// The [`ServiceExt`] combinators can be used to extend with additional middleware.
///
/// # Example
///
/// ```
/// use burger::*;
///
/// let middleware = MiddlewareBuilder.concurrency_limit(3).buffer(2).load_shed();
/// let svc = service_fn(|x: u32| async move { x.to_string() });
/// let svc = middleware.apply(svc);
/// ```
#[derive(Debug, Clone)]
pub struct MiddlewareBuilder;

impl Service<Infallible> for MiddlewareBuilder {
    type Response = Infallible;

    async fn acquire(&self) -> impl AsyncFnOnce(Infallible) -> Self::Response {
        async |request| request
    }
}

impl<S> Middleware<S> for MiddlewareBuilder {
    type Service = S;

    fn apply(self, svc: S) -> Self::Service {
        svc
    }
}
