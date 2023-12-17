use futures_util::{stream::FuturesUnordered, StreamExt};
use indexmap::IndexMap;

use crate::Service;

use super::Load;

/// Panics if empty.
pub struct Balance<S, Key> {
    services: IndexMap<Key, S>,
}

impl<Request, S, Key> Service<Request> for Balance<S, Key>
where
    S: Service<Request>,
    S: Load,
    Key: 'static,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        let mut permits: FuturesUnordered<_> = self
            .services
            .values()
            .map(|s| async {
                let permit = s.acquire().await;
                let load = s.load();
                (load, permit)
            })
            .collect();
        let (first_load, first_permit) = permits.next().await.expect("at least one service");
        if let Some((second_load, second_permit)) = permits.next().await {
            if first_load < second_load {
                first_permit
            } else {
                second_permit
            }
        } else {
            first_permit
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        S::call(permit, request).await
    }
}
