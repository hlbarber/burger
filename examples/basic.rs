use burger::{service_fn, ServiceExt};
use futures_util::join;

#[tokio::main]
async fn main() {
    let svc = service_fn(|()| async {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        "foo"
    })
    .concurrency_limit(2)
    .map(|val| println!("{val}"));
    join! {
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
    };
}
