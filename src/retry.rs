use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use pin_project_lite::pin_project;

use crate::{oneshot::Oneshot, Service, ServiceExt};

pub trait Policy<S, Request>
where
    S: Service<Request>,
{
    type RequestState<'a>;
    type Future<'a>: Future<Output = Result<(), Request>>
    where
        Self: 'a;
    type Error<'a>
    where
        Self: 'a;

    fn create(&self, request: &Request) -> (Self::RequestState<'_>, Request);

    fn classify(
        &self,
        state: &mut Self::RequestState<'_>,
        request: &Request,
        response: &<S::Future<'_> as Future>::Output,
    ) -> Self::Future<'_>;

    fn error(&self, state: Self::RequestState<'_>) -> Self::Error<'_>;
}

pub struct Retry<S, P> {
    inner: S,
    policy: P,
}

impl<S, P> Retry<S, P> {
    pub fn new(inner: S, policy: P) -> Self {
        Self { inner, policy }
    }
}

pin_project! {
    pub struct RetryFuture<'a, S, P, Request>
    where
        S: Service<Request>,
        P: Policy<S, Request>
    {
        service: &'a S,
        policy: &'a P,
        #[pin]
        inner: RetryFutureInner<'a, S, P, Request>,
        request: Request
    }
}

pin_project! {
    #[project = RetryFutureInnerProj]
    #[project_replace = RetryFutureInnerProjReplace]
    enum RetryFutureInner<'a, S: 'a, P: 'a, Request>
    where
        P: Policy<S, Request>,
        S: Service<Request>,
    {
        Initial { permit: S::Permit<'a> },
        Calling {
            #[pin] call: S::Future<'a>,
            state: P::RequestState<'a>,
        },
        Retrying {
            #[pin]
            oneshot: Oneshot<'a, S, Request>,
            state: P::RequestState<'a>,
        },
        Classifying {
            #[pin] classify: P::Future<'a>,
            state: P::RequestState<'a>,
            response: <S::Future<'a> as Future>::Output
        },
        Pending
    }
}

impl<'a, S, P, Request> Future for RetryFuture<'a, S, P, Request>
where
    S: Service<Request>,
    P: Policy<S, Request>,
{
    type Output = Result<<S::Future<'a> as Future>::Output, P::Error<'a>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut().project();
        loop {
            let next = match this.inner.as_mut().project() {
                RetryFutureInnerProj::Initial { .. } => {
                    let RetryFutureInnerProjReplace::Initial { permit } = this
                        .inner
                        .as_mut()
                        .project_replace(RetryFutureInner::Pending)
                    else {
                        unreachable!()
                    };
                    let (state, request) = this.policy.create(&this.request);
                    RetryFutureInner::Calling {
                        call: S::call(permit, request),
                        state,
                    }
                }
                RetryFutureInnerProj::Calling { call, .. } => {
                    let response = ready!(call.poll(cx));
                    let RetryFutureInnerProjReplace::Calling { mut state, .. } = this
                        .inner
                        .as_mut()
                        .project_replace(RetryFutureInner::Pending)
                    else {
                        unreachable!()
                    };
                    let classify = this.policy.classify(&mut state, &this.request, &response);
                    RetryFutureInner::Classifying {
                        classify,
                        state,
                        response,
                    }
                }
                RetryFutureInnerProj::Retrying { oneshot, .. } => {
                    let response = ready!(oneshot.poll(cx));
                    let RetryFutureInnerProjReplace::Retrying { mut state, .. } = this
                        .inner
                        .as_mut()
                        .project_replace(RetryFutureInner::Pending)
                    else {
                        unreachable!()
                    };
                    let classify = this.policy.classify(&mut state, &this.request, &response);
                    RetryFutureInner::Classifying {
                        classify,
                        state,
                        response,
                    }
                }
                RetryFutureInnerProj::Classifying { classify, .. } => {
                    let result = ready!(classify.poll(cx));
                    match result {
                        Ok(_) => {
                            let RetryFutureInnerProjReplace::Classifying { response, .. } = this
                                .inner
                                .as_mut()
                                .project_replace(RetryFutureInner::Pending)
                            else {
                                unreachable!()
                            };
                            return Poll::Ready(Ok(response));
                        }
                        Err(request) => {
                            let oneshot = this.service.oneshot(request);
                            let state = {
                                let RetryFutureInnerProjReplace::Classifying { state, .. } = this
                                    .inner
                                    .as_mut()
                                    .project_replace(RetryFutureInner::Pending)
                                else {
                                    unreachable!()
                                };
                                state
                            };
                            RetryFutureInner::Retrying { oneshot, state }
                        }
                    }
                }
                RetryFutureInnerProj::Pending => unreachable!(),
            };
            this.inner.as_mut().project_replace(next);
        }
    }
}

pub struct RetryPermit<'a, S, P, Inner> {
    service: &'a S,
    policy: &'a P,
    inner: Inner,
}

pin_project! {
    pub struct RetryAcquire<'a, S, P, Inner> {
        service: &'a S,
        policy: &'a P,
        #[pin]
        inner: Inner,
    }
}

impl<'a, S, P, Inner> Future for RetryAcquire<'a, S, P, Inner>
where
    Inner: Future,
{
    type Output = RetryPermit<'a, S, P, Inner::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();
        let inner = ready!(this.inner.poll(cx));
        let permit = RetryPermit {
            service: self.service,
            policy: self.policy,
            inner,
        };
        Poll::Ready(permit)
    }
}

impl<Request, S, P> Service<Request> for Retry<S, P>
where
    S: Service<Request>,
    P: Policy<S, Request>,
{
    type Future<'a> = RetryFuture<'a, S, P, Request>
    where
        Self: 'a;

    type Permit<'a> = RetryPermit<'a, S, P, S::Permit<'a>>
    where
        Self: 'a;

    type Acquire<'a> = RetryAcquire<'a, S, P, S::Acquire<'a>>
    where
        Self: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        RetryAcquire {
            service: &self.inner,
            policy: &self.policy,
            inner: self.inner.acquire(),
        }
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        RetryFuture {
            service: permit.service,
            policy: permit.policy,
            inner: RetryFutureInner::Initial {
                permit: permit.inner,
            },
            request,
        }
    }
}
