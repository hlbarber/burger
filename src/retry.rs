use crate::{Service, ServiceExt};

pub trait Policy<S, Request>
where
    S: Service<Request>,
{
    type RequestState<'a>;
    type Error;

    fn create(&self, request: &Request) -> Self::RequestState<'_>;

    async fn classify<'a>(
        &self,
        state: Self::RequestState<'a>,
        response: &S::Response<'_>,
    ) -> Result<Option<(Request, Self::RequestState<'a>)>, Self::Error>;
}

pub struct Retry<S, P> {
    inner: S,
    policy: P,
}

impl<S, P> Retry<S, P> {
    pub(crate) fn new(inner: S, policy: P) -> Self {
        Self { inner, policy }
    }
}

pub struct RetryPermit<'a, S, P, Inner> {
    service: &'a S,
    policy: &'a P,
    inner: Inner,
}

impl<Request, S, P> Service<Request> for Retry<S, P>
where
    S: Service<Request>,
    P: Policy<S, Request>,
{
    type Response<'a> = Result<S::Response<'a>, P::Error>;
    type Permit<'a> = RetryPermit<'a, S, P, S::Permit<'a>>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        RetryPermit {
            service: &self.inner,
            policy: &self.policy,
            inner: self.inner.acquire().await,
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response<'a> {
        let RetryPermit {
            service,
            policy,
            inner,
        } = permit;
        let mut state = policy.create(&request);
        let mut response = S::call(inner, request).await;

        loop {
            let Some((request, new_state)) = policy.classify(state, &response).await? else {
                return Ok(response);
            };
            state = new_state;
            response = service.oneshot(request).await;
        }
    }
}
