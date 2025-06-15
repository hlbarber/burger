//! The [`ServiceExt::retry`] combinator returns [`Retry`], which retries following a specified
//! [`Policy`].
//!
//! A [`Policy`] is used to instantiate the per request state and classify whether a request was
//! successful or not.
//!
//! The [`Service::acquire`] on [`Retry`] waits to acquire the inner [`Service::Permit`]. The
//! [`Service::call`] then:
//!
//! 1. Calls [`Policy::create`] to produce [`Policy::RequestState`].
//! 2. Uses the inner permit to [`Service::call`] the inner [`Service`].
//! 3. Calls [`Policy::classify`], with the [`Policy::RequestState`] from (1).
//! 4. If [`Ok`] then returns the [`Service::Response`], if [`Err`] then returns retries using
//!     [`ServiceExt::oneshot`] to obtain the next permit.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! use http::{Request, Response};
//!
//! struct RetryServiceUnavailable;
//!
//! struct State<BReq>(Request<BReq>);
//!
//! impl<S, BReq, BResp> retry::Policy<S, Request<BReq>> for RetryServiceUnavailable
//! where
//!     S: Service<Request<BReq>, Response = Response<BResp>> + 'static,
//!     BReq: Clone,
//! {
//!     type RequestState<'a> = State<BReq>;
//!
//!     fn create(&self, request: &Request<BReq>) -> State<BReq> {
//!         State(request.clone())
//!     }
//!
//!     async fn classify<'a>(
//!         &self,
//!         mut state: State<BReq>,
//!         response: Response<BResp>,
//!     ) -> Result<Response<BResp>, (Request<BReq>, State<BReq>)> {
//!         if response.status() != http::StatusCode::SERVICE_UNAVAILABLE {
//!             return Ok(response);
//!         }
//!         if let Some(some) = response.headers().get("retry-after") {
//!             Err((state.0.clone(), state))
//!         } else {
//!             Ok(response)
//!         }
//!     }
//! }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [`Retry`] defers to the inner service.

use crate::{load::Load, Middleware, Service, ServiceExt};

/// A retry policy allows for customization of [Retry].
///
/// # Example
///
/// ```rust
/// use burger::*;
/// use http::{Request, Response};
///
/// struct FiniteRetries(usize);
///
/// struct Attempts<'a, BReq> {
///     max: &'a usize,
///     request: Request<BReq>,
///     attempted: usize,
/// }
///
/// impl<S, BReq, BResp> retry::Policy<S, Request<BReq>> for FiniteRetries
/// where
///     S: Service<Request<BReq>, Response = http::Response<BResp>>,
///     BReq: Clone,
/// {
///     type RequestState<'a> = Attempts<'a, BReq>;
///
///     fn create(&self, request: &Request<BReq>) -> Self::RequestState<'_> {
///         Attempts {
///             max: &self.0,
///             request: request.clone(),
///             attempted: 0,
///         }
///     }
///
///     async fn classify<'a>(
///         &self,
///         mut state: Self::RequestState<'a>,
///         response: Response<BResp>,
///     ) -> Result<Response<BResp>, (Request<BReq>, Self::RequestState<'a>)> {
///         if response.status() == http::status::StatusCode::OK {
///             return Ok(response);
///         }
///
///         state.attempted += 1;
///         if state.attempted >= *state.max {
///             return Ok(response);
///         }
///
///         Err((state.request.clone(), state))
///     }
/// }
/// ```
pub trait Policy<S, Request>
where
    S: Service<Request>,
{
    /// The type of the request state.
    type RequestState<'a>;

    /// Creates a new [RequestState](Policy::RequestState).
    fn create(&self, request: &Request) -> Self::RequestState<'_>;

    /// Classifies the response, determining whether it was successful. On success returns [Ok]
    /// [`Service::Response`], on failure returns the next request and the updated
    /// [`Policy::RequestState`].
    async fn classify<'a>(
        &self,
        state: Self::RequestState<'a>,
        response: S::Response,
    ) -> Result<S::Response, (Request, Self::RequestState<'a>)>;
}

/// A wrapper for the [`ServiceExt::retry`] combinator.
///
/// See the [module](crate::retry) for more information.
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

impl<Request, S, P> Service<Request> for Retry<S, P>
where
    S: Service<Request>,
    P: Policy<S, Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let permit = self.inner.acquire().await;
        async |request| {
            let mut state = self.policy.create(&request);
            let mut response = permit(request).await;
            loop {
                match self.policy.classify(state, response).await {
                    Ok(response) => return response,
                    Err((request, new_state)) => {
                        state = new_state;
                        response = self.inner.oneshot(request).await;
                    }
                }
            }
        }
    }
}

impl<S, P> Load for Retry<S, P>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S, T, P> Middleware<S> for Retry<T, P>
where
    T: Middleware<S>,
{
    type Service = Retry<T::Service, P>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, policy } = self;
        Retry {
            inner: inner.apply(svc),
            policy,
        }
    }
}
