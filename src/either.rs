//! Often we want the branches of a runtime condition to output different service types. The
//! [`Either`] [`Service`] allows the reconciliation of two separate types. The [`Either::Left`]
//! and [`Either::Right`] variants can be constructed by
//! [`ServiceExt::left`](crate::ServiceExt::left) and
//! [`ServiceExt::right`](crate::ServiceExt::right) respectively.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//!
//! # let condition = true;
//! let svc = service_fn(|x| x + 2);
//! let svc = if let Some(some) = max_concurrency {
//!     svc.concurrency_limit(some).load_shed().left()
//! } else {
//!     svc.load_shed().right()
//! };
//! let response = svc.oneshot(10u32).await;
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [`Either`] defers to the variant.

use crate::{load::Load, Service};

/// A wrapper [`Service`] for [ServiceExt::left](crate::ServiceExt::left) and
/// [ServiceExt::right](crate::ServiceExt::right) which consolidates two types.
///
/// See the [module](mod@crate::either) for more information.
#[derive(Debug)]
pub enum Either<A, B> {
    #[allow(missing_docs)]
    Left(A),
    #[allow(missing_docs)]
    Right(B),
}

impl<Request, A, B> Service<Request> for Either<A, B>
where
    A: Service<Request>,
    B: Service<Request, Response = A::Response>,
{
    type Response = A::Response;
    type Permit<'a> = Either<A::Permit<'a>, B::Permit<'a>>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        match self {
            Either::Left(left) => Either::Left(left.acquire().await),
            Either::Right(right) => Either::Right(right.acquire().await),
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        match permit {
            Either::Left(permit) => A::call(permit, request).await,
            Either::Right(permit) => B::call(permit, request).await,
        }
    }
}

impl<A, B> Load for Either<A, B>
where
    A: Load,
    B: Load<Metric = A::Metric>,
{
    type Metric = A::Metric;

    fn load(&self) -> Self::Metric {
        match self {
            Either::Left(left) => left.load(),
            Either::Right(right) => right.load(),
        }
    }
}
