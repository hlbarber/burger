// mod boxed;
// mod buffer;
mod limit;
mod load_shed;
mod map;
mod oneshot;
mod service_fn;
mod then;

pub use limit::*;
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

    fn call<'a>(guard: Self::Permit<'a>, request: Request) -> Self::Future<'a>;
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
        Then {
            inner: self,
            closure,
        }
    }

    fn map<F>(self, closure: F) -> Map<Self, F>
    where
        Self: Sized,
    {
        Map {
            inner: self,
            closure,
        }
    }

    fn concurrency_limit(self, n_permits: usize) -> ConcurrencyLimit<Self>
    where
        Self: Sized,
    {
        ConcurrencyLimit {
            inner: self,
            semaphore: async_lock::Semaphore::new(n_permits),
        }
    }
}

impl<S, Request> ServiceExt<Request> for S where S: Service<Request> {}
