use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::future::Ready;
use tokio::sync::{
    mpsc::{error::TrySendError, Permit},
    oneshot,
};

use crate::Service;

pub struct Buffer<S, Request>
where
    S: Service<Request> + 'static,
{
    sender: tokio::sync::mpsc::Sender<BufferItem<Request, <S::Future<'static> as Future>::Output>>,
    inner: S,
}

pub struct BufferAcquireError;

pub struct BufferPermit<'a, Inner, Request, Response> {
    inner: Inner,
    reserved: Permit<'a, BufferItem<Request, Response>>,
}

pin_project_lite::pin_project! {
    #[project = BufferAcquireInnerProj]
    enum BufferAcquireInner<'a, Inner, Item> {
        Success {
            #[pin]
            inner: Inner,
            reserved: Permit<'a, Item>
        },
        Failure
    }
}

struct BufferItem<Request, Response> {
    request: Request,
    sender: oneshot::Sender<Response>,
}

pin_project_lite::pin_project! {
    pub struct BufferAcquire<'a, Inner, Request, Response> {
        #[pin]
        inner: BufferAcquireInner<'a, Inner, BufferItem<Request, Response>>,
    }
}

impl<'a, Inner, Request, Response> Future for BufferAcquire<'a, Inner, Request, Response>
where
    Inner: Future,
{
    type Output = Result<BufferPermit<'a, Inner::Output, Request, Response>, BufferAcquireError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.inner.project() {
            BufferAcquireInnerProj::Success { inner, reserved } => todo!(),
            BufferAcquireInnerProj::Failure => Poll::Ready(Err(BufferAcquireError)),
        }
    }
}

type ReserveResult<'a, Request> = Result<Permit<'a, Request>, TrySendError<()>>;

impl<S, Request> Service<Request> for Buffer<S, Request>
where
    S: Service<Request> + 'static,
{
    type Future<'a> = S::Future<'a>
    where
        Self: 'a;

    type Permit<'a> = Result<BufferPermit<'a, S::Permit<'a>, Request, <S::Future<'a> as Future>::Output>, BufferAcquireError>
    where
        Self: 'a;

    type Acquire<'a> = BufferAcquire<'a, S::Acquire<'a>, Request, <S::Future<'a> as Future>::Output>
    where
        Self: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        BufferAcquire {
            inner: match self.sender.try_reserve() {
                Ok(reserved) => BufferAcquireInner::Success {
                    inner: self.inner.acquire(),
                    reserved,
                },
                Err(_) => BufferAcquireInner::Failure,
            },
        }
    }

    fn call<'a>(guard: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        todo!()
    }
}
