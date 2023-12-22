use std::{
    future::poll_fn,
    sync::{Mutex, MutexGuard},
};

use tower_service::Service as TowerService;

use crate::{balance::Load, Service};

#[derive(Debug)]
pub struct Compat<S> {
    inner: Mutex<S>,
}

impl<Request, S> Service<Request> for Compat<S>
where
    S: TowerService<Request>,
{
    type Response = Result<S::Response, S::Error>;
    type Permit<'a> = Result<MutexGuard<'a, S>, S::Error>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        poll_fn(|cx| self.inner.lock().unwrap().poll_ready(cx))
            .await
            .map(|_| self.inner.lock().unwrap())
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        // https://github.com/rust-lang/rust-clippy/issues/6446
        let fut = {
            let mut guard = permit?;
            guard.call(request)
        };
        fut.await
    }
}

pub fn compat<S>(inner: S) -> Compat<S> {
    Compat {
        inner: Mutex::new(inner),
    }
}

impl<S> Load for Compat<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.lock().unwrap().load()
    }
}
