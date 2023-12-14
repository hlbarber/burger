use futures_util::future::select_all;

use crate::Service;

pub struct Select<I> {
    services: I,
}

impl<Request, I, S> Service<Request> for Select<I>
where
    for<'a> &'a I: IntoIterator<Item = &'a S> + 'a,
    I: 'static,
    S: Service<Request, acquire(): Unpin> + 'static,
{
    type Response<'a> = S::Response<'a>;
    type Permit<'a> = S::Permit<'a>
    where
        I: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        let iter = self.services.into_iter().map(|s| s.acquire());
        let (permit, _, _) = select_all(iter).await;
        permit
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response<'_> {
        S::call(permit, request).await
    }
}

pub fn select<I: IntoIterator>(services: I) -> Select<I> {
    Select { services }
}
