use std::fmt;

use futures_util::future::join_all;

use crate::Service;

#[derive(Debug)]
pub struct Steer<S, P> {
    services: Box<[S]>,
    picker: P,
}

trait Picker<S, Request> {
    fn pick(&self, services: &[S], request: &Request) -> usize;
}

pub struct SteerPermit<'a, S, P, Request>
where
    S: Service<Request>,
{
    services: &'a [S],
    permits: Vec<S::Permit<'a>>,
    picker: &'a P,
}

impl<'a, S, P, Request> fmt::Debug for SteerPermit<'a, S, P, Request>
where
    S: Service<Request> + fmt::Debug,
    S::Permit<'a>: fmt::Debug,
    P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SteerPermit")
            .field("services", &self.services)
            .field("permits", &self.permits)
            .field("picker", &self.picker)
            .finish()
    }
}

impl<Request, S, P> Service<Request> for Steer<S, P>
where
    S: Service<Request>,
    P: Picker<S, Request>,
{
    type Response = S::Response;
    type Permit<'a> = SteerPermit<'a, S, P, Request>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        SteerPermit {
            services: &self.services,
            picker: &self.picker,
            permits: join_all(self.services.iter().map(|x| x.acquire())).await,
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        let SteerPermit {
            services,
            mut permits,
            picker,
        } = permit;
        let index = picker.pick(services, &request);
        let permit = permits.swap_remove(index);
        drop(permits);
        S::call(permit, request).await
    }
}

pub fn steer<S, P>(services: impl IntoIterator<Item = S>, picker: P) -> Steer<S, P> {
    Steer {
        services: services.into_iter().collect(),
        picker,
    }
}
