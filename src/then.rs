use std::{any, fmt, future::Future};

use crate::Service;

#[derive(Debug, Clone)]
pub struct Then<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Then<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

pub struct ThenPermit<'a, S, F, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    closure: &'a F,
}

impl<'a, S, F, Request> fmt::Debug for ThenPermit<'a, S, F, Request>
where
    S: Service<Request>,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThenPermit")
            .field("inner", &self.inner)
            .field("closure", &format_args!("{}", any::type_name::<F>()))
            .finish()
    }
}

impl<Request, S, F, Fut> Service<Request> for Then<S, F>
where
    S: Service<Request>,
    F: Fn(S::Response) -> Fut,
    Fut: Future,
{
    type Response = Fut::Output;
    type Permit<'a> = ThenPermit<'a, S, F, Request> where S: 'a, F: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        ThenPermit {
            inner: self.inner.acquire().await,
            closure: &self.closure,
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        (permit.closure)(S::call(permit.inner, request).await).await
    }
}
