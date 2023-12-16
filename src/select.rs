use std::{fmt, marker::PhantomData};

use futures_util::future::select_all;

use crate::Service;

pub struct Select<S, I> {
    _inner: PhantomData<S>,
    services: I,
}

impl<S, I> fmt::Debug for Select<S, I>
where
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Select")
            .field("_inner", &self._inner)
            .field("services", &self.services)
            .finish()
    }
}

impl<S, I> Clone for Select<S, I>
where
    I: Clone,
{
    fn clone(&self) -> Self {
        Self {
            _inner: self._inner,
            services: self.services.clone(),
        }
    }
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
