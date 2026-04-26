# Service Bus Module

Purpose: abstract inter-module communication transport.

## Implementation

The `llmos-service-bus` crate in `crates/service-bus/` provides:

- `Transport` trait for publish/subscribe messaging
- `Envelope` and `MessageId` for transport-agnostic message framing
- `LocalChannel` in-process implementation backed by tokio mpsc channels

## Responsibilities
- Define service discovery model
- Version transport schemas
- Allow gRPC/NATS replacement without module rewrites
