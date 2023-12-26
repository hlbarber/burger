pub mod p2c;

#[derive(Debug)]
pub enum Change<K, V> {
    Insert(K, V),
    Remove(K),
}
