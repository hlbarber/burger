use std::{any, fmt, future::Future};

use crate::Service;

#[derive(Clone)]
pub struct ServiceFn<F> {
    closure: F,
}

impl<F> fmt::Debug for ServiceFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceFn")
            .field("closure", &format_args!("{}", any::type_name::<F>()))
            .finish()
    }
}

impl<Request, Fut, F> Service<Request> for ServiceFn<F>
where
    F: Fn(Request) -> Fut,
    Fut: Future,
{
    type Response = Fut::Output;
    type Permit<'a> = &'a F where F: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        &self.closure
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        permit(request).await
    }
}

pub fn service_fn<Request, Fut, F: Fn(Request) -> Fut>(closure: F) -> ServiceFn<F> {
    ServiceFn { closure }
}
