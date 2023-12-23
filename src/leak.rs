use std::{fmt, sync::Arc};

use crate::{balance::Load, Service};

#[derive(Debug)]
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

impl<'t, S, Request> fmt::Debug for LeakPermit<'t, S, Request>
where
    S: Service<Request> + fmt::Debug,
    for<'a> S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeakPermit")
            .field("_svc", &self._svc)
            .field("inner", &self.inner)
            .finish()
    }
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
        S::call(permit.inner, request).await
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
