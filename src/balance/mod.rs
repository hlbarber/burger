mod guard;
mod p2c;

pub trait Load {
    type Metric: PartialOrd;

    fn load(&self) -> Self::Metric;
}

pub enum Change<K, V> {
    Insert(K, V),
    Remove(K),
}
