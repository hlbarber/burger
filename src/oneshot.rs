use crate::Service;

pub async fn oneshot<Request, S>(request: Request, service: &S) -> S::Response<'_>
where
    S: Service<Request>,
{
    let permit = service.acquire().await;
    S::call(permit, request).await
}

pub(crate) struct Depressurize<S> {
    pub(crate) inner: S,
}

impl<Request, S> Service<Request> for Depressurize<S>
where
    S: Service<Request>,
{
    type Response<'a> = S::Response<'a>;
    type Permit<'a> = &'a S
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        &self.inner
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response<'a> {
        oneshot(request, permit).await
    }
}
