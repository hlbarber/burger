use std::sync::atomic::{AtomicUsize, Ordering};

use burger::{retry::Policy, service_fn::service_fn, Service, ServiceExt};

struct FiniteRetries(usize);

struct Attempts<'a> {
    max: &'a usize,
    request: Request,
    attempted: usize,
}

#[derive(Debug)]
struct MaxAttempts;

#[derive(Clone)]
struct Request;

impl<S> Policy<S, Request> for FiniteRetries
where
    S: Service<Request, Response = Result<usize, MaxAttempts>>,
{
    type RequestState<'a> = Attempts<'a>;

    fn create(&self, request: &Request) -> Attempts {
        Attempts {
            max: &self.0,
            request: request.clone(),
            attempted: 0,
        }
    }

    async fn classify<'a>(
        &self,
        mut state: Self::RequestState<'a>,
        response: S::Response,
    ) -> Result<S::Response, (Request, Attempts<'a>)> {
        if let Ok(200) | Err(_) = response {
            return Ok(response);
        }

        state.attempted += 1;
        if state.attempted >= *state.max {
            return Ok(Err(MaxAttempts));
        }

        Err((state.request.clone(), state))
    }
}

#[tokio::main]
async fn main() {
    let counter = &AtomicUsize::new(198);
    let value = service_fn(|_request| async move { counter.fetch_add(1, Ordering::SeqCst) })
        .map(Ok)
        .retry(FiniteRetries(4))
        .oneshot(Request)
        .await
        .unwrap();
    println!("{value}");
}
