mod buffer;
mod limit;
mod load_shed;
mod map;
mod oneshot;
mod ready_cache;
mod service_fn;
mod then;

pub use buffer::*;
pub use limit::*;
pub use load_shed::*;
pub use map::*;
pub use oneshot::*;
pub use service_fn::*;
pub use then::*;

use std::future::Future;

pub trait Service<Request> {
    type Future<'a>: Future
    where
        Self: 'a;

    type Permit<'a>
    where
        Self: 'a;
    type Acquire<'a>: Future<Output = Self::Permit<'a>>
    where
        Self: 'a;

    fn acquire(&self) -> Self::Acquire<'_>;

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a>;
}

pub trait ServiceExt<Request>: Service<Request> {
    fn oneshot(&self, request: Request) -> Oneshot<'_, Self, Request>
    where
        Self: Sized,
    {
        oneshot(request, self)
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

    // fn rate_limit(self, interval: Duration, n_permits: usize) -> RateLimit<Self>
    // where
    //     Self: Sized,
    // {
    //     RateLimit::new(self, interval, n_permits)
    // }

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
}

impl<S, Request> ServiceExt<Request> for S where S: Service<Request> {}
