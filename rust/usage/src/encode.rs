//! The bytes of what is sent (metrics-plan m_55738: plain JSON, no OTLP):
//! a batch of packets for `POST /v1/packets`, the delete request for
//! `POST /v1/delete`. One small function each, apart from the sending and
//! the schema, so the PII law ([`crate::law::check_packets_body`]) reads
//! exactly these bytes. The transport may compress them; it never adds a
//! field.

use serde::Serialize;

use crate::install_id::InstallId;
use crate::packet::Packet;

/// The most packets one body carries (30 days of spool).
pub const MAX_BATCH: usize = 30;

/// The body of `POST /v1/packets`: `{"packets": [packet, …]}`, at most
/// [`MAX_BATCH`] (the caller splits a longer list).
pub fn packets(batch: &[Packet]) -> Vec<u8> {
    #[derive(Serialize)]
    struct Body<'a> {
        packets: &'a [Packet],
    }
    debug_assert!(batch.len() <= MAX_BATCH, "a batch of {} packets", batch.len());
    serde_json::to_vec(&Body { packets: batch }).expect("a packet serializes")
}

/// The body of `POST /v1/delete`: `{"install_id": "<uuid>"}`.
pub fn delete_request(id: &InstallId) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "install_id": id.reveal() })).expect("an id serializes")
}
