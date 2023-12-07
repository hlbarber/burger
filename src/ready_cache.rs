use futures_util::{
    future::{select_all, Map, SelectAll},
    FutureExt,
};

use crate::Service;

pub struct ReadyCache<I> {
    services: I,
}

impl<Request, I, S> Service<Request> for ReadyCache<I>
where
    for<'a> &'a I: IntoIterator<Item = &'a S>,
    I: 'static,
    S: Service<Request> + 'static,
    for<'a> S::Acquire<'a>: Unpin,
{
    type Future<'a> = S::Future<'a> 
    where
        I: 'a;
    type Permit<'a> = S::Permit<'a>
    where
        I: 'a;
    type Acquire<'a> = Map<SelectAll<S::Acquire<'a>>, fn((S::Permit<'a>, usize, Vec<S::Acquire<'a>>)) -> S::Permit<'a>>
    where
        I: 'a;

    fn acquire<'a>(&'a self) -> Self::Acquire<'a> {
        let iter = self.services.into_iter().map(|s| s.acquire());
        select_all(iter).map(|(permit, _, _)| permit)
    }

    fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        S::call(permit, request)
    }
}

pub fn ready_cache<I: IntoIterator>(services: I) -> ReadyCache<I> {
    ReadyCache { services }
}