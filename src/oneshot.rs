use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use pin_project_lite::pin_project;

use crate::Service;

pin_project! {
    #[project= OneshotInnerProj]
    #[project_replace = OneshotInnerProjReplace]
    enum OneshotInner<Request, Acquire, Call>
    {
        // TODO: Can we remove this `Option`?
        Acquire { request: Option<Request>, #[pin] inner: Acquire },
        Call { #[pin] inner: Call },
        Transition
    }
}

pin_project! {
    pub struct Oneshot<'a, S, Request>
    where
        S: Service<Request>,
    {
        service: &'a S,
        #[pin]
        state: OneshotInner<Request, S::Acquire<'a>, S::Future<'a>>,
    }
}

pub fn oneshot<Request, S>(request: Request, service: &S) -> Oneshot<'_, S, Request>
where
    S: Service<Request>,
{
    Oneshot {
        service,
        state: OneshotInner::Acquire {
            request: Some(request),
            inner: service.acquire(),
        },
    }
}

impl<'a, Request, S> Future for Oneshot<'a, S, Request>
where
    S: Service<Request>,
{
    type Output = <S::Future<'a> as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut().project();
        loop {
            let state = this.state.as_mut().project();
            let new_state = match state {
                OneshotInnerProj::Acquire { inner, request } => match inner.poll(cx) {
                    Poll::Ready(ready) => OneshotInner::Call {
                        inner: S::call(
                            ready,
                            request.take().expect("this cannot be taken more than once"),
                        ),
                    },
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                },
                OneshotInnerProj::Call { inner } => {
                    let output = ready!(inner.poll(cx));
                    return Poll::Ready(output);
                }
                OneshotInnerProj::Transition => {
                    unreachable!("this is an ephemeral state and cannot be reached")
                }
            };
            this.state.as_mut().project_replace(new_state);
        }
    }
}
