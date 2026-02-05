//! HTTP and WebSocket server

pub struct Server {
    port: u16,
}

impl Server {
    #[must_use]
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
}
