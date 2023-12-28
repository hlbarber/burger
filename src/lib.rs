#![allow(async_fn_in_trait)]
#![deny(missing_docs, missing_debug_implementations)]

//! An experimental service framework.
//!
//! The [`Service`] trait is the central abstraction. It is an
//! [asynchronous function](Service::call), accepting a request and returning a response, which
//! can only be executed _after_ a [permit](Service::Permit) is [acquired](Service::acquire).
//!
//! The root exports [`Service`] constructors, and an extension trait, [`ServiceExt`], which provides combinators
//! to modify a [`Service`]. Both the combinators and constructors each have an associated module
//! containing related documentation, traits, and types.
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
pub mod leak;
pub mod load;
pub mod load_shed;
pub mod map;
pub mod retry;
pub mod select;
pub mod service_fn;
pub mod steer;
pub mod then;

use std::sync::Arc;

use buffer::Buffer;
use concurrency_limit::ConcurrencyLimit;
use depressurize::Depressurize;
use either::Either;
use leak::Leak;
use load::{Load, PendingRequests};
use load_shed::LoadShed;
use map::Map;
use retry::Retry;
use then::Then;
use tokio::sync::{Mutex, RwLock};

#[doc(inline)]
pub use balance::p2c::balance as balance_p2c;
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
    /// The type of the permit required to call the service.
    type Permit<'a>
    where
        Self: 'a;

    /// Obtains a permit.
    async fn acquire(&self) -> Self::Permit<'_>;

    /// Consumes a permit to call the service.
    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a;
}

/// An extension trait for [`Service`].
pub trait ServiceExt<Request>: Service<Request> {
    /// Acquires the permit and then immediately uses it to call the service.
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
        let permit = self.acquire().await;
        Self::call(permit, request).await
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

    /// Extends the lifetime of the permit.
    ///
    /// See the [module](leak) for more information.
    fn leak<'t>(self: Arc<Self>) -> Leak<'t, Self>
    where
        Self: Sized,
    {
        Leak::new(self)
    }

    /// Wraps as [Either::Left]. Related to [right](ServiceExt::right).
    ///
    /// See the [module](either) for more information.
    fn left<T>(self) -> Either<Self, T>
    where
        Self: Sized,
    {
        Either::Left(self)
    }

    /// Wraps as [Either::Right]. Related to [left](ServiceExt::left).
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

impl<Request, S> Service<Request> for Arc<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        S::acquire(self).await
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        S::call(permit, request).await
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
    type Permit<'a> = S::Permit<'a>
    where
        S:'a, 't: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        S::acquire(self).await
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}

impl<'t, S> Load for &'t S
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        S::load(self)
    }
}

impl<Request, Permit, S> Service<Request> for Mutex<S>
where
    // NOTE: These bounds seem too tight
    for<'a> S: Service<Request, Permit<'a> = Permit>,
    S: 'static,
{
    type Response = S::Response;
    type Permit<'a> = Permit
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        let guard = self.lock().await;
        guard.acquire().await
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        S::call(permit, request).await
    }
}

impl<Request, S, Permit> Service<Request> for RwLock<S>
where
    // NOTE: These bounds seem too tight
    for<'a> S: Service<Request, Permit<'a> = Permit>,
    S: 'static,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.read().await.acquire().await
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        S::call(permit, request).await
    }
}
