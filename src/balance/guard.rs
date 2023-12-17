use left_right::ReadGuard;

use crate::Service;

impl<'rh, Request, S> Service<Request> for ReadGuard<'rh, S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        S::acquire(self).await
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}
