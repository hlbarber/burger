#![allow(async_fn_in_trait)]
// #![deny(missing_docs, missing_debug_implementations)]
#![deny(missing_debug_implementations)]

//! An experimental service framework.

pub mod balance;
pub mod buffer;
#[cfg(feature = "compat")]
pub mod compat;
pub mod concurrency_limit;
pub mod leak;
pub mod load_shed;
pub mod map;
pub mod oneshot;
pub mod retry;
pub mod select;
pub mod service_fn;
pub mod steer;
pub mod then;

use std::sync::Arc;

use balance::{Load, PendingRequests};
use buffer::Buffer;
use concurrency_limit::ConcurrencyLimit;
use load_shed::LoadShed;
use map::Map;
use oneshot::oneshot;
use retry::Retry;
use then::Then;
use tokio::sync::{Mutex, RwLock};

#[cfg(feature = "compat")]
pub use compat::compat;

pub trait Service<Request> {
    type Response;
    type Permit<'a>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_>;

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a;
}

pub trait ServiceExt<Request>: Service<Request> {
    async fn oneshot(&self, request: Request) -> Self::Response
    where
        Self: Sized,
    {
        oneshot(request, self).await
    }

    fn then<F>(self, closure: F) -> Then<Self, F>
    where
        Self: Sized,
    {
        Then::new(self, closure)
    }

    fn map<F>(self, closure: F) -> Map<Self, F>
    where
        Self: Sized,
    {
        Map::new(self, closure)
    }

    fn concurrency_limit(self, n_permits: usize) -> ConcurrencyLimit<Self>
    where
        Self: Sized,
    {
        ConcurrencyLimit::new(self, n_permits)
    }

    fn load_shed(self) -> LoadShed<Self>
    where
        Self: Sized,
    {
        LoadShed::new(self)
    }

    fn buffer(self, capacity: usize) -> Buffer<Self>
    where
        Self: Sized,
    {
        Buffer::new(self, capacity)
    }

    fn retry<P>(self, policy: P) -> Retry<Self, P>
    where
        Self: Sized,
    {
        Retry::new(self, policy)
    }

    fn pending_requests(self) -> PendingRequests<Self>
    where
        Self: Sized,
    {
        PendingRequests::new(self)
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
