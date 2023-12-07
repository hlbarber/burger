use std::future::{ready, Future, Ready};

use futures_util::{
    future::{Either, Map},
    FutureExt,
};

use crate::Service;

pub struct LoadShed<S> {
    inner: S,
}

impl<S> LoadShed<S> {
    pub(crate) fn new(inner: S) -> Self {
        LoadShed { inner }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct Shed;

type NewOutput<'a, S, Request> =
    Result<<<S as Service<Request>>::Future<'a> as Future>::Output, Shed>;

impl<Request, S> Service<Request> for LoadShed<S>
where
    S: Service<Request>,
{
    type Future<'a> = Either<
        Map<S::Future<'a>, fn(<S::Future<'a> as Future>::Output) -> NewOutput<'a, S, Request>>,
        Ready<NewOutput<'a, S, Request>>
    >
    where
        Self: 'a;

    type Permit<'a> = Option<S::Permit<'a>>
    where
        Self: 'a;

    type Acquire<'a> = Ready<Self::Permit<'a>>
    where
        Self: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        ready(self.inner.acquire().now_or_never())
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        if let Some(permit) = permit {
            Either::Left(S::call(permit, request).map(Ok))
        } else {
            Either::Right(ready(Err(Shed)))
        }
    }
}
