//! Ports — abstract contracts that the domain exposes for driven adapters.
//!
//! A *port* is an inversion-of-control interface declared by the inner
//! layers (domain / application) and implemented by driven adapters
//! living in infrastructure crates. The domain declares the *intent*
//! (what must be delivered), never the transport (HTTP, gRPC, queue,
//! file, etc.).
//!
//! The trait methods here are intentionally **synchronous**. Driven
//! adapters may use async runtimes internally; the composition root
//! bridges sync → async via runtime handles or `spawn_blocking`.

mod notification;

pub use notification::{
    NotificationDeliveryError, NotificationRequest, NotificationSink, NotificationTarget,
    NullNotificationSink,
};
