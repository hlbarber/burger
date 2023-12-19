use std::{future::Future, hash::Hash, pin::pin, sync::Arc};

use futures_util::{stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use indexmap::IndexMap;
use tokio::sync::Mutex;

use crate::{
    leak::{leak, Leak, LeakPermit},
    Service,
};

use super::{Change, Load};

pub struct Balance<S, Key> {
    inner: Arc<Mutex<BalanceInner<Leak<'static, S>, Key>>>,
    empty: Arc<Mutex<()>>,
}

impl<Request, S, Key> Service<Request> for Balance<S, Key>
where
    S: Service<Request> + Load + 'static,
    Key: Eq + Hash + 'static,
{
    type Response = S::Response;
    type Permit<'a> = LeakPermit::<'static, S, Request>
    where
        Self: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.inner.acquire().await
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        Leak::call(permit, request).await
    }
}

pub fn balance<St, Key, S>(changes: St) -> (Balance<S, Key>, impl Future<Output = ()>)
where
    St: Stream<Item = Change<Key, S>>,
    Key: Eq + Hash,
{
    let inner = Arc::new(Mutex::new(BalanceInner {
        services: IndexMap::new(),
    }));
    let empty = Arc::new(Mutex::new(()));
    let balance = Balance {
        inner: inner.clone(),
        empty: empty.clone(),
    };
    let fut = async move {
        let mut changes = pin!(changes);
        while let Some(mut new_change) = changes.next().await {
            let mut guard = inner.lock().await;
            loop {
                match new_change {
                    Change::Insert(key, service) => guard.insert(key, leak(Arc::new(service))),
                    Change::Remove(key) => guard.remove(&key),
                };
                let Some(change) = changes.next().now_or_never().flatten() else {
                    break;
                };
                new_change = change;
            }
        }
    };

    (balance, fut)
}

/// Panics if empty.
struct BalanceInner<S, Key> {
    services: IndexMap<Key, S>,
}

impl<S, Key> BalanceInner<S, Key>
where
    Key: Eq + Hash,
{
    fn insert(&mut self, key: Key, service: S) -> Option<S> {
        self.services.insert(key, service)
    }

    fn remove(&mut self, key: &Key) -> Option<S> {
        self.services.remove(key)
    }

    fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

impl<Request, S, Key> Service<Request> for BalanceInner<S, Key>
where
    S: Service<Request> + Load,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        S: 'a, Key: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        let mut permits: FuturesUnordered<_> = self
            .services
            .values()
            .map(|s| async {
                let permit = s.acquire().await;
                let load = s.load().await;
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

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}
