//! Various load balancer implementations.

pub mod p2c;

/// Represents a change to a pool of [services](crate::Service).
#[derive(Debug)]
pub enum Change<K, V> {
    /// Inserts a service into the collection.
    Insert(K, V),
    /// Removes a service from the collection.
    Remove(K),
}
