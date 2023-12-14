use crate::Service;

pub struct Map<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Map<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

pub struct MapPermit<'a, Inner, F> {
    inner: Inner,
    closure: &'a F,
}

impl<Request, S, F, Output> Service<Request> for Map<S, F>
where
    S: Service<Request>,
    for<'a> F: Fn(S::Response<'a>) -> Output,
{
    type Response<'a> = Output;
    type Permit<'a> = MapPermit<'a, S::Permit<'a>, F> where S: 'a, F: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        MapPermit {
            inner: self.inner.acquire().await,
            closure: &self.closure,
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response<'a> {
        (permit.closure)(S::call(permit.inner, request).await)
    }
}
