use serde_json::json;

use crate::channel::LocalChannel;
use crate::message::Envelope;
use crate::transport::{Transport, TransportError};

#[tokio::test]
async fn publish_delivers_to_subscriber() {
    let bus = LocalChannel::new();
    let mut rx = bus.subscribe("events").await.unwrap();

    let msg = Envelope::new("events", "test-source", json!({"key": "value"}));
    bus.publish(msg).await.unwrap();

    let received = rx.recv().await.expect("should receive message");
    assert_eq!(received.topic, "events");
    assert_eq!(received.source, "test-source");
    assert_eq!(received.payload, json!({"key": "value"}));
}

#[tokio::test]
async fn publish_to_topic_with_no_subscribers_returns_error() {
    let bus = LocalChannel::new();
    let msg = Envelope::new("nowhere", "test-source", json!(null));
    let err = bus.publish(msg).await.unwrap_err();
    assert!(matches!(err, TransportError::NoSubscribers(_)));
}

#[tokio::test]
async fn multiple_subscribers_each_receive_a_copy() {
    let bus = LocalChannel::new();
    let mut rx1 = bus.subscribe("topic-a").await.unwrap();
    let mut rx2 = bus.subscribe("topic-a").await.unwrap();

    let msg = Envelope::new("topic-a", "src", json!(42));
    bus.publish(msg).await.unwrap();

    let m1 = rx1.recv().await.expect("subscriber 1");
    let m2 = rx2.recv().await.expect("subscriber 2");
    assert_eq!(m1.payload, json!(42));
    assert_eq!(m2.payload, json!(42));
}

#[tokio::test]
async fn message_id_is_unique() {
    let a = Envelope::new("t", "s", json!(null));
    let b = Envelope::new("t", "s", json!(null));
    assert_ne!(a.id, b.id);
}

#[tokio::test]
async fn separate_topics_are_isolated() {
    let bus = LocalChannel::new();
    let mut rx_a = bus.subscribe("topic-a").await.unwrap();
    let mut rx_b = bus.subscribe("topic-b").await.unwrap();

    bus.publish(Envelope::new("topic-a", "s", json!("a")))
        .await
        .unwrap();
    bus.publish(Envelope::new("topic-b", "s", json!("b")))
        .await
        .unwrap();

    let a = rx_a.recv().await.unwrap();
    let b = rx_b.recv().await.unwrap();
    assert_eq!(a.payload, json!("a"));
    assert_eq!(b.payload, json!("b"));
}
