use std::{ops::Range, sync::Arc, time::Duration};

use burger::{balance, service_fn, ServiceExt};
use rand::Rng;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    const N_SERVICES: u64 = 4;
    const LATENCY_RANGE_MS: Range<u64> = 10..250;

    let svc = balance::p2c().arc();
    let handle = svc.handle();

    for i in 0..N_SERVICES {
        let svc = service_fn(async move |_: ()| {
            let interval_ms = rand::thread_rng().gen_range(LATENCY_RANGE_MS);
            sleep(Duration::from_millis(interval_ms)).await;
            println!("{i}")
        })
        .pending_requests()
        .arc();
        handle.add_service(i, svc);
    }

    for _ in 0..100 {
        let svc = svc.clone();
        tokio::spawn(svc.oneshot(()));
    }

    loop {
        let load_profile = svc.load_profile().await;
        tracing::info!(?load_profile);

        let interval_ms = rand::thread_rng().gen_range(LATENCY_RANGE_MS);
        sleep(Duration::from_millis(interval_ms)).await;
    }
}
