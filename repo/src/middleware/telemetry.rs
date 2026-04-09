use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    time::Instant,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};

/// Actix-web middleware that records per-request HTTP metrics into the
/// Prometheus registry (`src/metrics.rs`):
///
/// - `http_requests_total{method, path, status}`
/// - `http_request_duration_seconds{method, path}`
/// - `http_errors_total{method, path, status}` — only for 4xx/5xx
///
/// Route patterns (e.g. `/users/{id}`) are used instead of raw paths to avoid
/// high-cardinality label explosion.
pub struct Telemetry;

impl<S, B> Transform<S, ServiceRequest> for Telemetry
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TelemetryMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TelemetryMiddleware { service }))
    }
}

pub struct TelemetryMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for TelemetryMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start = Instant::now();
        let method = req.method().to_string();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;

            let elapsed = start.elapsed().as_secs_f64();
            let status = res.status().as_u16().to_string();

            // Prefer the matched route pattern to avoid per-ID label cardinality
            let path = res
                .request()
                .match_pattern()
                .unwrap_or_else(|| res.request().path().to_string());

            crate::metrics::http_requests()
                .with_label_values(&[&method, &path, &status])
                .inc();

            crate::metrics::http_duration()
                .with_label_values(&[&method, &path])
                .observe(elapsed);

            let status_code = res.status().as_u16();
            if status_code >= 400 {
                crate::metrics::http_errors()
                    .with_label_values(&[&method, &path, &status])
                    .inc();
            }

            Ok(res)
        })
    }
}
