// This is used in `Select` to ensure `Unpin`.
#![feature(return_type_notation)]

pub mod buffer;
pub mod concurrency_limit;
pub mod load_shed;
pub mod map;
pub mod oneshot;
pub mod retry;
pub mod select;
pub mod service_fn;
pub mod then;

use buffer::Buffer;
use concurrency_limit::ConcurrencyLimit;
use load_shed::LoadShed;
use map::Map;
use oneshot::oneshot;
use retry::Retry;
use then::Then;

pub trait Service<Request> {
    type Response;
    type Permit<'a>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_>;

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response;
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
}

impl<S, Request> ServiceExt<Request> for S where S: Service<Request> {}
