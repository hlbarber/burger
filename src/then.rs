use std::future::Future;

use crate::Service;

pub struct Then<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Then<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

pub struct ThenPermit<'a, Inner, F> {
    inner: Inner,
    closure: &'a F,
}

impl<Request, S, F, Fut> Service<Request> for Then<S, F>
where
    S: Service<Request>,
    F: Fn(S::Response) -> Fut,
    Fut: Future,
{
    type Response = Fut::Output;
    type Permit<'a> = ThenPermit<'a, S::Permit<'a>, F> where S: 'a, F: 'a;

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
