//! The [ServiceExt::buffer](crate::ServiceExt::buffer) combinator returns [Buffer], whose
//! [Service::acquire] immediately resolves until the buffer is at maximum capacity, at which point
//! it defers to the inner service's [Service::acquire]. The buffer is drained when the inner
//! service's permit becomes available.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use tokio::{join, time::sleep};
//! # use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x| async move {
//!     sleep(Duration::from_secs(1)).await;
//!     x + 1
//! })
//! .concurrency_limit(1)
//! .buffer(2)
//! .load_shed();
//! let (a, b, c, d) = join! {
//!     svc.oneshot(9),
//!     svc.oneshot(2),
//!     svc.oneshot(1),
//!     svc.oneshot(5)
//! };
//! assert_eq!(a, Ok(10));
//! assert_eq!(b, Ok(3));
//! assert_eq!(c, Ok(2));
//! assert_eq!(d, Err(5));
//! # }
//! ```
//!
//! # Load
//!
//! The [Load::load] on [Buffer] defers to the inner service.

use std::fmt;

use futures_util::FutureExt;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{load::Load, Service};

/// A wrapper [Service] for the [ServiceExt::buffer](crate::ServiceExt::buffer) combinator.
///
/// See the [module](crate::buffer) for more information.
#[derive(Debug)]
pub struct Buffer<S> {
    inner: S,
    semaphore: Semaphore,
}

impl<S> Buffer<S> {
    pub(crate) fn new(inner: S, capacity: usize) -> Self {
        Self {
            inner,
            semaphore: Semaphore::new(capacity),
        }
    }
}

/// The [Service::Permit] type for [Buffer].
pub struct BufferPermit<'a, S, Request>
where
    S: Service<Request>,
{
    inner: BufferPermitInner<'a, S, Request>,
}

impl<'a, S, Request> fmt::Debug for BufferPermit<'a, S, Request>
where
    S: Service<Request>,
    BufferPermitInner<'a, S, Request>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferPermit")
            .field("inner", &self.inner)
            .finish()
    }
}

enum BufferPermitInner<'a, S, Request>
where
    S: Service<Request>,
{
    Eager(S::Permit<'a>),
    Buffered(&'a S, SemaphorePermit<'a>),
}

impl<'a, S, Request> fmt::Debug for BufferPermitInner<'a, S, Request>
where
    S: Service<Request> + fmt::Debug,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eager(arg0) => f.debug_tuple("Eager").field(arg0).finish(),
            Self::Buffered(arg0, arg1) => {
                f.debug_tuple("Buffered").field(arg0).field(arg1).finish()
            }
        }
    }
}

impl<Request, S> Service<Request> for Buffer<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = BufferPermit<'a, S, Request>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        BufferPermit {
            inner: match self.inner.acquire().now_or_never() {
                Some(some) => BufferPermitInner::Eager(some),
                None => BufferPermitInner::Buffered(
                    &self.inner,
                    self.semaphore.acquire().await.expect("not closed"),
                ),
            },
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        let permit = match permit.inner {
            BufferPermitInner::Eager(permit) => permit,
            BufferPermitInner::Buffered(service, _permit) => {
                let permit = service.acquire().await;
                drop(_permit);
                permit
            }
        };
        S::call(permit, request).await
    }
}

impl<S> Load for Buffer<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
