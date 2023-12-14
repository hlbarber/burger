use std::future::Future;

use crate::Service;

pub struct ServiceFn<F> {
    closure: F,
}

impl<Request, Fut, F> Service<Request> for ServiceFn<F>
where
    F: Fn(Request) -> Fut,
    Fut: Future,
{
    type Response<'a> = Fut::Output;
    type Permit<'a> = &'a F where F: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        &self.closure
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response<'a> {
        permit(request).await
    }
}

pub fn service_fn<Request, Fut, F: Fn(Request) -> Fut>(closure: F) -> ServiceFn<F> {
    ServiceFn { closure }
}
