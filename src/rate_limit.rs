//! The [`ServiceExt::rate_limit`](crate::ServiceExt::rate_limit) combinator returns [`RateLimit`],
//! which limits the number of [`Service::call`]s invoked per period of time.
//!
//! This implementation requires the number of permits and the interval is specified, each
//! [`Service::acquire`] acquires a permit, when [`Service::call`] is invoked the permit is
//! forgotten. The number of available permits is refreshed when the period has elapsed.
//!
//! Note that this does _not_ garauntee that a remote server will receive requests under these
//! restrictions. Network conditions, other middleware, etc can cause requests to arrive in bursts
//! exceeding the rate limit specified here.
//!
//! # Example
//!
//! If 5 permits and a interval of 2 second is specified then the first 5 [`Service::acquire`]s will
//! immediately resolve and the 6th will resolve after the 2 second interval has elapsed.
//!
//! ```rust
//! use std::time::Duration;
//!
//! use burger::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc =
//!     service_fn(|x: u32| async move { x.to_string() }).rate_limit(Duration::from_secs(1), 5);
//! let response = svc.oneshot(1).await;
//! # let _ = response;
//! # }
//! ```
use std::time::{Duration, Instant};

use tokio::{
    select,
    sync::{Mutex, Semaphore, SemaphorePermit},
};

use crate::Service;

/// A wrapper for the [`ServiceExt::rate_limit`](crate::ServiceExt::rate_limit) combinator.
///
/// See the [module](crate::rate_limit) for more information.
#[derive(Debug)]
pub struct RateLimit<S> {
    inner: S,
    semaphore: Semaphore,
    last_update: Mutex<Instant>,
    interval: Duration,
    permits: usize,
}

impl<S> RateLimit<S> {
    pub(crate) fn new(inner: S, interval: Duration, permits: usize) -> Self {
        Self {
            inner,
            semaphore: Semaphore::new(permits),
            last_update: Mutex::new(Instant::now()),
            interval,
            permits,
        }
    }
}

/// The [`Service::Permit`] type for [`RateLimit`].
#[derive(Debug)]
pub struct RateLimitPermit<'a, S, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    _permit: SemaphorePermit<'a>,
}

impl<Request, S> Service<Request> for RateLimit<S>
where
    S: Service<Request>,
{
    type Response = S::Response;

    type Permit<'a> = RateLimitPermit<'a, S, Request>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        let fut = async move {
            let mut guard = self.last_update.lock().await;
            loop {
                let now = Instant::now();
                let end = *guard + self.interval;
                tokio::time::sleep_until(end.into()).await;

                // Remove all permits, then add new ones
                self.semaphore.forget_permits(usize::MAX);
                self.semaphore.add_permits(self.permits);
                *guard = now;
            }
        };
        let acquire = self.semaphore.acquire();
        let permit = select! { permit = acquire => { permit }, never = fut => { never } };

        RateLimitPermit {
            _permit: permit.unwrap(),
            inner: self.inner.acquire().await,
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        let RateLimitPermit { inner, _permit } = permit;
        _permit.forget();
        S::call(inner, request).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::{service_fn, ServiceExt};

    #[tokio::test]
    async fn limit() {
        let svc = service_fn(|x: u32| async move { x.to_string() })
            .rate_limit(Duration::from_millis(100), 2);
        let now = Instant::now();

        // 0, 1 happen instantly
        // 2, 3 called
        // Wait for 100ms
        // 4, 5 called
        // Wait for 100ms
        // 6 called
        for _ in 0..7 {
            svc.oneshot(1).await;
        }
        let elapsed = Instant::now()
            .checked_duration_since(now)
            .expect("time travel isnt possible");
        println!("{elapsed:?}");
        assert!(elapsed > Duration::from_millis(200));
    }
}
