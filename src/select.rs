use futures_util::future::select_all;

use crate::Service;

pub struct Select<I> {
    services: I,
}

impl<Request, I, S> Service<Request> for Select<I>
where
    for<'a> &'a I: IntoIterator<Item = &'a S>,
    I: 'static,
    S: Service<Request, acquire(): Unpin> + 'static,
{
    type Response<'a> = S::Response<'a>;
    type Permit<'a> = S::Permit<'a>
    where
        I: 'a;

    async fn acquire<'a>(&'a self) -> Self::Permit<'a> {
        let iter = self.services.into_iter().map(|s| s.acquire());
        let (permit, _, _) = select_all(iter).await;
        permit
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response<'a> {
        S::call(permit, request).await
    }
}

pub fn select<I: IntoIterator>(services: I) -> Select<I> {
    Select { services }
}
