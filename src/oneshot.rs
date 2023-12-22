use crate::{balance::Load, Service};

pub(super) async fn oneshot<Request, S>(request: Request, service: &S) -> S::Response
where
    S: Service<Request>,
{
    let permit = service.acquire().await;
    S::call(permit, request).await
}

#[derive(Clone, Debug)]
pub(crate) struct Depressurize<S> {
    pub(crate) inner: S,
}

impl<Request, S> Service<Request> for Depressurize<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = &'a S
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        &self.inner
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        oneshot(request, permit).await
    }
}

impl<S> Load for Depressurize<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
