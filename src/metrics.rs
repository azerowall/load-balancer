use lazy_static::lazy_static;
use prometheus::{
    opts, register_histogram_vec, register_int_counter_vec, Encoder, HistogramVec, IntCounterVec,
    TextEncoder,
};

lazy_static! {
    pub static ref UPSTREAM_RPS_COUNT: IntCounterVec =
        register_int_counter_vec!(opts!("upstream_rps_count", "upstream rps"), &["host"])
            .expect("Can't create metric");
    pub static ref UPSTREAM_ERRORS_COUNT: IntCounterVec = register_int_counter_vec!(
        opts!("upstream_errors_count", "upstreams errors"),
        &["host", "reason"]
    )
    .expect("Can't create metric");
    pub static ref UPSTREAM_TIMINGS_SECONDS: HistogramVec = register_histogram_vec!(
        "upstream_timings_seconds",
        "upstream timings_seconds",
        &["host"],
        vec![0.0001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
    )
    .expect("Can't create metric");
}

pub fn gather() -> crate::Result<Vec<u8>> {
    let mut buffer = Vec::new();

    let encoder = TextEncoder::new();
    encoder.encode(&prometheus::gather(), &mut buffer)?;

    Ok(buffer)
}
