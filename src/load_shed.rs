use futures_util::FutureExt;

use crate::Service;

#[derive(Clone, Debug)]
pub struct LoadShed<S> {
    inner: S,
}

impl<S> LoadShed<S> {
    pub(crate) fn new(inner: S) -> Self {
        LoadShed { inner }
    }
}

#[derive(Debug)]
pub struct Shed<Request>(pub Request);

impl<Request, S> Service<Request> for LoadShed<S>
where
    S: Service<Request>,
{
    type Response = Result<S::Response, Shed<Request>>;
    type Permit<'a> = Option<S::Permit<'a>>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.inner.acquire().now_or_never()
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        if let Some(permit) = permit {
            Ok(S::call(permit, request).await)
        } else {
            Err(Shed(request))
        }
    }
}
