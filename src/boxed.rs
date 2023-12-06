use std::future::Future;

use crate::Service;

type DefaultDyn<Request, Output> =
    dyn for<'a> Service<Request, Future<'a> = Box<dyn Future<Output = Output>>>;

// pub struct Boxed<Request, Output, Dyn = dyn Service<Request, >> {
//     inner: Box<Dyn>
// }
