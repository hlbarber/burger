//! Various load balancer implementations.

pub mod p2c;

/// Represents a change to a pool of [services](crate::Service).
#[derive(Debug)]
pub enum Change<K, V> {
    Insert(K, V),
    Remove(K),
}
