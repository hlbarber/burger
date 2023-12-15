use std::{
    future::poll_fn,
    sync::{Mutex, MutexGuard},
};

use tower_service::Service as TowerService;

use crate::Service;

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
        let fut = {
            let mut guard = permit?;
            guard.call(request)
        };
        fut.await
    }
}

pub fn compact<S>(inner: S) -> Compat<S> {
    Compat {
        inner: Mutex::new(inner),
    }
}