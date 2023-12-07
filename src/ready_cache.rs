use futures_util::{
    future::{select_all, Map, SelectAll},
    FutureExt,
};

use crate::Service;

pub struct ReadyCache<S> {
    services: Vec<S>,
}

impl<Request, S> Service<Request> for ReadyCache<S>
where
    S: Service<Request>,
    for<'a> S::Acquire<'a>: Unpin,
{
    type Future<'a> = S::Future<'a>
    where
        S: 'a;

    type Permit<'a> = S::Permit<'a>
    where
       S: 'a;

    type Acquire<'a> = Map<SelectAll<S::Acquire<'a>>, fn((S::Permit<'a>, usize, Vec<S::Acquire<'a>>)) -> S::Permit<'a>>
    where
        S: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        let iter = self.services.iter().map(|s| s.acquire());
        select_all(iter).map(|(permit, _, _)| permit)
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        S::call(permit, request)
    }
}
