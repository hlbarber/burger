An experimental service framework.

## FAQ

### Why `&self` rather than `&mut self`

We are optimizing the API for a multithreaded environment. The default case will use interior mutability, in which case `&mut self` is excessive.

### Why use permits?

Permits allow you to [disarm](https://github.com/tower-rs/tower/issues/408) a service after it's ready.

### Why doesn't `call` accept `&self`?

The readiness of one service does not ensure the readiness of a different service of the same type - we want to disallow sharing of permits. There are three options here:

1. Pass the innards required for `call` from `Self` to the permit (by reference).
2. Use some sort of [branding](https://plv.mpi-sws.org/rustbelt/ghostcell/). This adds a lot of complexity.
3. Ignore the problem - implementors can implement runtime checks to prevent sharing if they wish.

Choosing 1. is safe and less obscure than 2..

### Why is `Service::Permit<'a>` a GAT?

We want to be able to pass the innards `&self` into the `Self::Permit<'_>` by reference. Cloning `Arc`s from the `&self` to `Self::Permit` will result in poor performance and developer experience.

### Why not a synchronous permit acquisition, like `tower::Service::poll_ready`?

Using `Future` here allows for easy composition with the large `Future`s ecosystem. Both approaches boil down to the same kind of state machines eventually.

### Why is `Service::Response<'a>` a GAT?

Often performance conscious users want to take a buffer and deserialize it into a borrowed type. Rust does not have good support for self-referential types and so, without the `<'a>` on `Service::Response`, users would not be able to go from owned types to borrowed types.

### Why do `async fn acquire` and `async fn call` not return a `Result`?

Most of the `Service` style combinators work without `Result`.

If the user wants to write a `Service` with a fallible `async fn acquire` then they can model the permit as a `Result` and have `call` return the `Err`.

### Why does `call` take ownership of the permit?

A permit should provide one `call` as standard. Specific implementations may introduce clone to the permit.