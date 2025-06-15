use std::time::Duration;

use burger::{select::select, service_fn, ServiceExt};
use futures_util::future::join_all;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let svcs: Vec<_> = (0..2)
        .into_iter()
        .map(|i| {
            service_fn(async move |_: ()| {
                sleep(Duration::from_secs(1)).await;
                println!("{i}");
            })
            .concurrency_limit(1)
        })
        .collect();
    let svc = select(svcs);
    join_all([
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
    ])
    .await;
}
