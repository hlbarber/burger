use std::time::Duration;

use burger::{select::select, Service, ServiceExt};
use futures_util::future::join_all;
use tokio::time::sleep;

struct Svc(usize);

impl Service<()> for Svc {
    type Response = ();
    type Permit<'a> = usize;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.0
    }

    async fn call<'a>(permit: Self::Permit<'a>, _request: ()) -> Self::Response {
        sleep(Duration::from_secs(1)).await;
        println!("{permit}");
    }
}

#[tokio::main]
async fn main() {
    let svc = select(vec![
        Svc(0).concurrency_limit(1),
        Svc(1).concurrency_limit(1),
        Svc(2).concurrency_limit(1),
    ]);
    join_all([
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
        svc.oneshot(()),
    ])
    .await;
}
