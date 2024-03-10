use burger::{service_fn, Middleware, MiddlewareBuilder, ServiceExt};

#[tokio::main]
async fn main() {
    // Construct middleware
    let middleware = MiddlewareBuilder.concurrency_limit(3).buffer(2).load_shed();

    let svc = service_fn(|x: String| async move { format!("hello, {x}") });
    // Apply middleware
    let svc = middleware.apply(svc);

    // Result is a `Service`
    let _y = svc.oneshot("world".to_string()).await.unwrap();
}
