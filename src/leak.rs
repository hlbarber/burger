use std::sync::Arc;

use crate::{balance::Load, Service};

pub struct Leak<'t, S> {
    _ref: &'t (),
    inner: Arc<S>,
}

pub fn leak<'t, S>(inner: Arc<S>) -> Leak<'t, S> {
    Leak { _ref: &(), inner }
}

pub struct LeakPermit<'t, S, Request>
where
    S: Service<Request> + 't,
{
    _svc: Arc<S>,
    inner: S::Permit<'t>,
}

impl<'t, Request, S> Service<Request> for Leak<'t, S>
where
    S: Service<Request> + 't,
{
    type Response = S::Response;
    type Permit<'a> = LeakPermit<'t, S, Request>
    where
        S: 'a, 't: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        LeakPermit {
            _svc: self.inner.clone(),
            inner: unsafe { std::mem::transmute(self.inner.acquire().await) },
        }
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        let response = S::call(permit.inner, request).await;
        response
    }
}

impl<'t, S> Load for Leak<'t, S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
