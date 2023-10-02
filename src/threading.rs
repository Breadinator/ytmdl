use once_cell::sync::Lazy;
use rayon::{ThreadPool, ThreadPoolBuilder};

pub static POOL: Lazy<ThreadPool> = Lazy::new(|| ThreadPoolBuilder::new().build().unwrap());
