use std::sync::atomic::{AtomicUsize, Ordering};

use burger::{service_fn, Policy, Service, ServiceExt};

struct FiniteRetries(usize);

struct Attempts<'a> {
    max: &'a usize,
    attempted: usize,
}

#[derive(Debug)]
struct MaxAttempts;

impl<S> Policy<S, ()> for FiniteRetries
where
    for<'a> S: Service<(), Response<'a> = usize>,
{
    type RequestState<'a> = Attempts<'a>;
    type Error = MaxAttempts;

    fn create(&self, _request: &()) -> Attempts {
        Attempts {
            max: &self.0,
            attempted: 0,
        }
    }

    async fn classify<'a>(
        &self,
        mut state: Self::RequestState<'a>,
        response: &<S as Service<()>>::Response<'_>,
    ) -> Result<Option<((), Self::RequestState<'a>)>, Self::Error> {
        if *response == 200 {
            return Ok(None);
        }

        state.attempted += 1;

        if state.attempted >= *state.max {
            return Err(MaxAttempts);
        }

        Ok(Some(((), state)))
    }
}

#[tokio::main]
async fn main() {
    let x = &AtomicUsize::new(198);
    service_fn(|()| async move {
        
        x.fetch_add(1, Ordering::SeqCst)
    })
    .retry(FiniteRetries(4))
    .oneshot(())
    .await
    .unwrap();
}
