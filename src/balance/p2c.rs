use std::{
    future::Future,
    hash::Hash,
    ops::{Deref, DerefMut},
    pin::pin,
    sync::Arc,
};

use futures_util::{future::join_all, stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use indexmap::IndexMap;
use tokio::sync::{OwnedRwLockWriteGuard, RwLock, RwLockWriteGuard};

use crate::{
    leak::{leak, Leak, LeakPermit},
    Service,
};

use super::{Change, Load};

pub struct Balance<S, Key> {
    inner: Arc<RwLock<BalanceInner<Leak<'static, S>, Key>>>,
}

impl<S, Key> Balance<S, Key>
where
    S: Load,
{
    pub async fn load_profile(&self) -> Vec<S::Metric> {
        join_all(
            self.inner
                .read()
                .await
                .services
                .values()
                .map(|svc| svc.load()),
        )
        .await
    }
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

enum EitherLock<'a, T> {
    Borrowed(RwLockWriteGuard<'a, T>),
    Owned(OwnedRwLockWriteGuard<T>),
}

impl<T> Deref for EitherLock<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            EitherLock::Borrowed(x) => x.deref(),
            EitherLock::Owned(x) => x.deref(),
        }
    }
}
impl<T> DerefMut for EitherLock<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            EitherLock::Borrowed(x) => x.deref_mut(),
            EitherLock::Owned(x) => x.deref_mut(),
        }
    }
}

pub fn balance<St, Key, S>(changes: St) -> (Balance<S, Key>, impl Future<Output = ()>)
where
    St: Stream<Item = Change<Key, S>>,
    Key: Eq + Hash,
{
    let inner = Arc::new(RwLock::new(BalanceInner {
        services: IndexMap::new(),
    }));
    let balance = Balance {
        inner: inner.clone(),
    };
    let empty_guard = inner.clone().try_write_owned().unwrap();
    let fut = async move {
        let mut empty_guard = Some(EitherLock::Owned(empty_guard));
        let mut changes = pin!(changes);
        while let Some(mut new_change) = changes.next().await {
            let mut guard = if let Some(guard) = empty_guard.take() {
                guard
            } else {
                EitherLock::Borrowed(inner.write().await)
            };
            loop {
                match new_change {
                    Change::Insert(key, service) => {
                        guard.insert(key, leak(Arc::new(service)));
                    }
                    Change::Remove(key) => {
                        guard.remove(&key);
                    }
                };
                let Some(change) = changes.next().now_or_never().flatten() else {
                    if guard.is_empty() {
                        empty_guard = Some(guard);
                    } else {
                        let _guard = empty_guard.take();
                        drop(_guard);
                    }
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
            .map(|s| async move {
                let permit = s.acquire().await;
                (s, permit)
            })
            .collect();
        let (first, first_permit) = permits.next().await.unwrap();
        if let Some((second, second_permit)) = permits.next().now_or_never().flatten() {
            let first_load = first.load().await;
            let second_load = second.load().await;
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
