use std::time::Duration;

use burger::{service_fn, ServiceExt};
use futures_util::join;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let svc = service_fn(|()| async {
        sleep(Duration::from_secs(5)).await;
        "foo"
    })
    .then(|output| async move { output })
    .concurrency_limit(1)
    .buffer(2)
    .load_shed();

    let x = join! {
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
    };
    println!("{x:?}");
}
