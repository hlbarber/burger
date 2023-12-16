use std::fmt;

use crate::{Service, ServiceExt};

pub trait Policy<S, Request>
where
    S: Service<Request>,
{
    type RequestState<'a>;

    fn create(&self, request: &Request) -> Self::RequestState<'_>;

    async fn classify<'a>(
        &self,
        state: Self::RequestState<'a>,
        response: S::Response,
    ) -> Result<S::Response, (Request, Self::RequestState<'a>)>;
}

#[derive(Clone, Debug)]
pub struct Retry<S, P> {
    inner: S,
    policy: P,
}

impl<S, P> Retry<S, P> {
    pub(crate) fn new(inner: S, policy: P) -> Self {
        Self { inner, policy }
    }
}

pub struct RetryPermit<'a, S, P, Request>
where
    S: Service<Request>,
{
    service: &'a S,
    policy: &'a P,
    inner: S::Permit<'a>,
}

impl<'a, S, P, Request> fmt::Debug for RetryPermit<'a, S, P, Request>
where
    S: Service<Request> + fmt::Debug,
    P: fmt::Debug,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetryPermit")
            .field("service", &self.service)
            .field("policy", &self.policy)
            .field("inner", &self.inner)
            .finish()
    }
}

impl<Request, S, P> Service<Request> for Retry<S, P>
where
    S: Service<Request>,
    P: Policy<S, Request>,
{
    type Response = S::Response;
    type Permit<'a> = RetryPermit<'a, S, P, Request>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        RetryPermit {
            service: &self.inner,
            policy: &self.policy,
            inner: self.inner.acquire().await,
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        let RetryPermit {
            service,
            policy,
            inner,
        } = permit;
        let mut state = policy.create(&request);
        let mut response = S::call(inner, request).await;

        loop {
            match policy.classify(state, response).await {
                Ok(response) => return response,
                Err((request, new_state)) => {
                    state = new_state;
                    response = service.oneshot(request).await;
                }
            }
        }
    }
}
