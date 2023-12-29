use std::{ops::Range, time::Duration};

use burger::{
    balance::{p2c::balance, Change},
    service_fn::service_fn,
    ServiceExt,
};
use futures_util::{
    stream::{iter, FuturesUnordered},
    StreamExt,
};
use rand::{random, Rng};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    const N_SERVICES: u64 = 4;
    const LATENCY_RANGE_MS: Range<u64> = 10..250;

    let stream = iter(0..N_SERVICES).map(|index| {
        let latency_ms = 3 * N_SERVICES * rand::thread_rng().gen_range(LATENCY_RANGE_MS);
        tracing::info!(index, latency_ms);
        let latency = Duration::from_millis(latency_ms);
        let svc = service_fn(move |_| sleep(latency))
            .concurrency_limit(1)
            .pending_requests();
        Change::Insert(index, svc)
    });

    let (svc, worker) = balance(stream);
    let _ = worker.await;

    let mut futures_unordered = FuturesUnordered::new();
    for _ in 0..100 {
        let load_profile = svc.load_profile().await;
        tracing::info!(?load_profile);

        let interval_ms = rand::thread_rng().gen_range(LATENCY_RANGE_MS);
        sleep(Duration::from_millis(interval_ms)).await;
        futures_unordered.push(svc.oneshot(()));

        // Using `FuturesUnordered` and randomly driving execution is a way to work around
        // https://github.com/rust-lang/rust/issues/100013
        if random() {
            futures_unordered.next().await;
        }
    }
}
