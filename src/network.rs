use std::net::TcpStream;
use std::time::Duration;

/// Shared HTTP client with timeouts so a slow or dead network can never hang a worker.
pub fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
}

#[allow(dead_code)]
pub fn is_connected() -> bool {
    TcpStream::connect_timeout(
        &"1.1.1.1:53".parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok()
}
