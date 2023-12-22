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

    const N_SERVICES: usize = 4;
    const LATENCY_RANGE: Range<f32> = 0.01..0.5;

    let stream = iter(0..N_SERVICES).map(|index| {
        let latency = N_SERVICES as f32 * 3.0 * rand::thread_rng().gen_range(LATENCY_RANGE);
        let latency = Duration::from_secs_f32(latency);
        let svc = service_fn(move |_| sleep(latency))
            .concurrency_limit(1)
            .pending_requests();
        Change::Insert(index, svc)
    });

    let (svc, worker) = balance(stream);
    worker.await;

    let mut futures_unordered = FuturesUnordered::new();
    loop {
        let profile = svc.load_profile().await;
        tracing::info!(?profile);

        let interval = rand::thread_rng().gen_range(LATENCY_RANGE);
        let interval = Duration::from_secs_f32(interval);
        sleep(interval).await;
        futures_unordered.push(svc.oneshot(()));
        if random() {
            futures_unordered.next().await;
        }
    }
}
