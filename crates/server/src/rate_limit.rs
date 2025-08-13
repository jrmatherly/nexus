//! Rate limiting middleware for HTTP requests.

use std::{
    fmt::Display,
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{body::Body, extract::ConnectInfo};
use http::{Request, Response, StatusCode};
use rate_limit::{RateLimitError, RateLimitManager, RateLimitRequest};
use tower::Layer;

use config::ClientIdentity;

#[derive(Clone)]
pub struct RateLimitLayer(Arc<RateLimitManager>);

impl RateLimitLayer {
    pub fn new(manager: Arc<RateLimitManager>) -> Self {
        Self(manager)
    }
}

impl<Service> Layer<Service> for RateLimitLayer
where
    Service: Send + Clone,
{
    type Service = RateLimitService<Service>;

    fn layer(&self, next: Service) -> Self::Service {
        RateLimitService {
            next,
            manager: self.0.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitService<Service> {
    next: Service,
    manager: Arc<RateLimitManager>,
}

impl<Service, ReqBody> tower::Service<Request<ReqBody>> for RateLimitService<Service>
where
    Service: tower::Service<Request<ReqBody>, Response = Response<Body>> + Send + Clone + 'static,
    Service::Future: Send,
    Service::Error: Display + 'static,
    ReqBody: http_body::Body + Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = Service::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.next.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut next = self.next.clone();
        let manager = self.manager.clone();

        Box::pin(async move {
            // Extract client IP for IP-based rate limiting
            let ip = extract_client_ip(&req);

            // Get client identity from request extensions (already validated by ClientIdentificationLayer)
            let identity = req.extensions().get::<ClientIdentity>().cloned();

            // Build rate limit request with IP and client identity
            let mut builder = RateLimitRequest::builder();

            if let Some(ip) = ip {
                builder = builder.ip(ip);
            }

            // Log client identity if present
            if let Some(ref identity) = identity {
                log::debug!(
                    "Rate limiting for client: {} in group: {:?}",
                    identity.client_id,
                    identity.group
                );
            }

            let rate_limit_request = builder.build();

            // Check rate limits
            let err = match manager.check_request(&rate_limit_request).await {
                Ok(()) => {
                    // Request allowed, continue to next handler
                    return next.call(req).await;
                }
                Err(err) => err,
            };

            // Log the specific rate limit error for debugging
            log::debug!("Request rejected due to rate limit: {err:?}");

            // Request blocked, return generic error without specific details
            let (status, message) = match &err {
                RateLimitError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
                _ => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded"),
            };

            let response = Response::builder()
                .status(status)
                .header("Content-Type", "text/plain")
                .body(Body::from(message))
                .unwrap();

            // No Retry-After headers are sent to maintain consistency with downstream LLM providers
            Ok(response)
        })
    }
}

/// Extract client IP address from request.
fn extract_client_ip<B>(req: &Request<B>) -> Option<IpAddr> {
    // First try to get from ConnectInfo (direct connection)
    if let Some(connect_info) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return Some(connect_info.0.ip());
    }

    // Try X-Forwarded-For header (for proxied requests)
    if let Some(forwarded_for) = req.headers().get("x-forwarded-for") {
        let value = forwarded_for.to_str().ok()?;

        // Take the first IP in the chain
        let ip_str = value.split(',').next()?;

        return ip_str.trim().parse::<IpAddr>().ok();
    }

    // Try X-Real-IP header
    let ip_str = req.headers().get("x-real-ip")?.to_str().ok()?;

    ip_str.parse::<IpAddr>().ok()
}
