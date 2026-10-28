use std::net::TcpStream;
use std::time::Duration;

pub fn is_connected() -> bool {
    TcpStream::connect_timeout(
        &"1.1.1.1:53".parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok()
}
