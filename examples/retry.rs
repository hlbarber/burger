use std::{
    future::{ready, Future, Ready},
    sync::atomic::{AtomicUsize, Ordering},
};

use burger::{service_fn, Policy, Service, ServiceExt};

struct FiniteRetries {
    max_retries: usize,
}
struct Attempts(usize);

#[derive(Debug)]
struct MaxAttempts;

impl<S, Request> Policy<S, Request> for FiniteRetries
where
    S: Service<Request>,
    // https://github.com/rust-lang/rust/issues/49601
    // for<'a> S::Future<'a>: Future<Output = &'a str>,
    for<'a> S::Future<'a>: Future<Output = usize>,
    Request: Clone,
{
    type RequestState<'a> = Attempts;
    type Future<'a> = Ready<Result<(), Request>>;
    type Error<'a> = MaxAttempts;

    fn create(&self, request: &Request) -> (Attempts, Request) {
        (Attempts(self.max_retries), request.clone())
    }

    fn classify(
        &self,
        state: &mut Attempts,
        request: &Request,
        response: &<<S as Service<Request>>::Future<'_> as futures_util::Future>::Output,
    ) -> Self::Future<'_> {
        let result = if *response != 200 && state.0.checked_sub(1).is_none() {
            Ok(())
        } else {
            Err(request.clone())
        };
        ready(result)
    }

    fn error(&self, _state: Attempts) -> Self::Error<'_> {
        MaxAttempts
    }
}

#[tokio::main]
async fn main() {
    let x = AtomicUsize::new(198);
    let svc = service_fn(move |()| {
        // let x = x;
        let x = &x;
        async move {
            let y = x.fetch_add(1, Ordering::SeqCst);
            println!("{y}");
            y
        }
    });
    let svc = svc.retry(FiniteRetries { max_retries: 5 });
    let x = svc.oneshot(()).await.unwrap();
    drop(x);
}
