use std::time::Duration;

use burger::{service_fn, ServiceExt};
use futures_util::join;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let svc = service_fn(|()| async {
        sleep(Duration::from_secs(1)).await;
        "foo "
    })
    .map(|output: &str| output.trim().to_string())
    .concurrency_limit(1)
    .buffer(2)
    .load_shed();

    let x = join! {
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
    };
    println!("{x:?}");
}
