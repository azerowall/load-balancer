use lazy_static::lazy_static;
use prometheus::{
    opts, register_histogram_vec, register_int_counter_vec, Encoder, HistogramVec, IntCounterVec,
    TextEncoder,
};

lazy_static! {
    pub static ref UPSTREAM_RPS: IntCounterVec =
        register_int_counter_vec!(opts!("upstream_rps_count", "upstream rps"), &["host"])
            .expect("Can't create metric");
    pub static ref UPSTREAM_ERRORS: IntCounterVec = register_int_counter_vec!(
        opts!("upstream_errors_count", "upstreams errors"),
        &["host", "reason"]
    )
    .expect("Can't create metric");
    pub static ref UPSTREAM_TIMINGS: HistogramVec = register_histogram_vec!(
        "upstream_timings",
        "upstream timings",
        &["host"],
        vec![0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0],
    )
    .expect("Can't create metric");
}

pub fn gather() -> crate::Result<Vec<u8>> {
    let mut buffer = Vec::new();

    let encoder = TextEncoder::new();
    encoder.encode(&prometheus::gather(), &mut buffer)?;

    Ok(buffer)
}
