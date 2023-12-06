use std::future::{ready, Future, Ready};

use crate::Service;

pub struct ServiceFn<F> {
    closure: F,
}

impl<Request, Fut, F> Service<Request> for ServiceFn<F>
where
    F: Fn(Request) -> Fut,
    Fut: Future,
{
    type Future<'a> = Fut where F: 'a;
    type Permit<'a> = &'a F where F: 'a;
    type Acquire<'a> = Ready<&'a F> where F: 'a;

    fn acquire(&self) -> Self::Acquire<'_> {
        ready(&self.closure)
    }

    fn call<'a>(guard: Self::Permit<'a>, request: Request) -> Self::Future<'a> {
        guard(request)
    }
}

pub fn service_fn<Request, Fut, F: Fn(Request) -> Fut>(closure: F) -> ServiceFn<F> {
    ServiceFn { closure }
}
