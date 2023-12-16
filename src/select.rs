use std::marker::PhantomData;

use futures_util::future::select_all;

use crate::Service;

pub struct Select<S, I> {
    _inner: PhantomData<S>,
    services: I,
}

impl<Request, S, I> Service<Request> for Select<S, I>
where
    for<'a> &'a I: IntoIterator<Item = &'a S>,
    S: Service<Request, acquire(): Unpin>,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        S: 'a, I: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        let iter = self.services.into_iter().map(|s| s.acquire());
        let (permit, _, _) = select_all(iter).await;
        permit
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}

pub fn select<S, I>(services: I) -> Select<S, I>
where
    I: IntoIterator,
{
    Select {
        _inner: PhantomData,
        services,
    }
}
