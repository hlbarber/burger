use std::time::Duration;

use burger::{
    balance::{p2c::balance, Change, DiscoverExt},
    service_fn::service_fn,
    ServiceExt,
};
use futures_util::{stream::iter, StreamExt};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let stream = iter(0..100)
        .then(|index| async move {
            sleep(Duration::from_secs(1)).await;
            index
        })
        .map(|index| {
            let svc = service_fn(move |_x: ()| async move {
                sleep(Duration::from_secs(index)).await;
                println!("{index}");
            })
            .concurrency_limit(1);
            Change::Insert(index, svc)
        })
        .pending_requests();

    let (svc, worker) = balance(stream);
    tokio::spawn(worker);
    svc.oneshot(()).await;
}
