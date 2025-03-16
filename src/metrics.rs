use lazy_static::lazy_static;
use prometheus::{opts, register_int_counter_vec, Encoder, IntCounterVec, TextEncoder};

lazy_static! {
    pub static ref UPSTREAM_RPS: IntCounterVec =
        register_int_counter_vec!(opts!("upstream_rps", "upstream rps"), &["host"])
            .expect("Can't create metric");
    pub static ref UPSTREAM_ERRORS: IntCounterVec =
        register_int_counter_vec!(opts!("upstream_errors", "upstreams errors"), &["host"])
            .expect("Can't create metric");
}

pub fn gather() -> crate::Result<Vec<u8>> {
    let mut buffer = Vec::new();

    let encoder = TextEncoder::new();
    encoder.encode(&prometheus::gather(), &mut buffer)?;

    Ok(buffer)
}
