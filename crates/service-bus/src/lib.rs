mod channel;
mod message;
mod transport;

pub use channel::LocalChannel;
pub use message::{Envelope, MessageId};
pub use transport::{Transport, TransportError};

#[cfg(test)]
mod tests;
