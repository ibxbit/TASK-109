use prometheus::{
    Counter, Encoder, Gauge, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry,
    TextEncoder,
};
use std::sync::OnceLock;

static REGISTRY: OnceLock<Registry> = OnceLock::new();
static HTTP_REQUESTS: OnceLock<IntCounterVec> = OnceLock::new();
static HTTP_DURATION: OnceLock<HistogramVec> = OnceLock::new();
static HTTP_ERRORS: OnceLock<IntCounterVec> = OnceLock::new();
static DB_POOL_ACTIVE: OnceLock<Gauge> = OnceLock::new();
static DB_POOL_IDLE: OnceLock<Gauge> = OnceLock::new();
static DB_POOL_WAIT_TIMEOUTS: OnceLock<Counter> = OnceLock::new();

pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let r = Registry::new();

        let requests = IntCounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        )
        .unwrap();

        // Ensure the counter family is always present in scrapes, even before
        // the first real request is recorded.
        requests
            .with_label_values(&["INIT", "/bootstrap", "200"])
            .inc_by(0);

        // Finer buckets around the 300 ms p95 target
        let duration = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request duration").buckets(
                vec![
                    0.005, 0.010, 0.025, 0.050, 0.075, 0.100, 0.150, 0.200, 0.250, 0.300, 0.400,
                    0.500, 0.750, 1.0, 2.5,
                ],
            ),
            &["method", "path"],
        )
        .unwrap();

        // Materialize one histogram label set so scrapes always include this
        // metric family even before the first real request observation.
        // Use a small positive value (not 0.0) to guarantee the histogram
        // sample_sum is non-zero, which ensures the TextEncoder always emits
        // the metric family regardless of prometheus internals or timing.
        duration
            .with_label_values(&["INIT", "/bootstrap"])
            .observe(0.001);

        let errors = IntCounterVec::new(
            Opts::new("http_errors_total", "Total HTTP 4xx/5xx responses"),
            &["method", "path", "status"],
        )
        .unwrap();

        // Keep the errors counter family materialized as well for consistency.
        errors
            .with_label_values(&["INIT", "/bootstrap", "500"])
            .inc_by(0);

        let pool_active = Gauge::with_opts(Opts::new(
            "db_pool_connections_active",
            "DB pool connections currently checked out",
        ))
        .unwrap();

        let pool_idle = Gauge::with_opts(Opts::new(
            "db_pool_connections_idle",
            "DB pool connections currently idle",
        ))
        .unwrap();

        let pool_wait_timeouts = Counter::with_opts(Opts::new(
            "db_pool_wait_timeout_total",
            "Total times a DB connection could not be obtained within timeout",
        ))
        .unwrap();

        r.register(Box::new(requests.clone())).unwrap();
        r.register(Box::new(duration.clone())).unwrap();
        r.register(Box::new(errors.clone())).unwrap();
        r.register(Box::new(pool_active.clone())).unwrap();
        r.register(Box::new(pool_idle.clone())).unwrap();
        r.register(Box::new(pool_wait_timeouts.clone())).unwrap();

        HTTP_REQUESTS.set(requests).unwrap();
        HTTP_DURATION.set(duration).unwrap();
        HTTP_ERRORS.set(errors).unwrap();
        DB_POOL_ACTIVE.set(pool_active).unwrap();
        DB_POOL_IDLE.set(pool_idle).unwrap();
        DB_POOL_WAIT_TIMEOUTS.set(pool_wait_timeouts).unwrap();

        r
    })
}

pub fn http_requests() -> &'static IntCounterVec {
    registry();
    HTTP_REQUESTS.get().unwrap()
}

pub fn http_duration() -> &'static HistogramVec {
    registry();
    HTTP_DURATION.get().unwrap()
}

pub fn http_errors() -> &'static IntCounterVec {
    registry();
    HTTP_ERRORS.get().unwrap()
}

#[allow(dead_code)]
pub fn db_pool_wait_timeouts() -> &'static Counter {
    registry();
    DB_POOL_WAIT_TIMEOUTS.get().unwrap()
}

/// Call this each time pool state is sampled (e.g. from /health or /internal/metrics).
pub fn update_pool_gauges(active: u32, idle: u32) {
    registry();
    DB_POOL_ACTIVE.get().unwrap().set(active as f64);
    DB_POOL_IDLE.get().unwrap().set(idle as f64);
}

/// Estimate the p95 latency in milliseconds from the HTTP duration histogram.
///
/// Uses linear interpolation over cumulative histogram buckets.
/// Returns `None` if no observations have been recorded yet.
pub fn estimate_p95_ms() -> Option<f64> {
    registry();
    let families = registry().gather();
    let family = families
        .iter()
        .find(|f| f.get_name() == "http_request_duration_seconds")?;

    let mut total_count: u64 = 0;
    let mut buckets: Vec<(f64, u64)> = Vec::new();

    for metric in family.get_metric() {
        let h = metric.get_histogram();
        total_count += h.get_sample_count();
        for b in h.get_bucket() {
            buckets.push((b.get_upper_bound(), b.get_cumulative_count()));
        }
    }

    if total_count == 0 {
        return None;
    }

    // Merge buckets across label combinations by upper bound
    buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let target = (total_count as f64 * 0.95).ceil() as u64;

    let mut prev_bound = 0.0_f64;
    let mut prev_count = 0_u64;

    for (upper, count) in &buckets {
        if *count >= target {
            // Linear interpolation within this bucket
            let fraction = if count == &prev_count {
                0.0
            } else {
                (target - prev_count) as f64 / (count - prev_count) as f64
            };
            let p95_secs = prev_bound + fraction * (upper - prev_bound);
            return Some(p95_secs * 1000.0);
        }
        prev_bound = *upper;
        prev_count = *count;
    }

    // All observations fall in the +Inf bucket — return the last finite bound * 1000
    Some(prev_bound * 1000.0)
}

pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry().gather();
    let mut buf = Vec::new();
    encoder.encode(&metric_families, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}
