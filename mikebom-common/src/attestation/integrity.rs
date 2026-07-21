use serde::{Deserialize, Serialize};

use crate::types::timestamp::Timestamp;

/// Diagnostic information about the fidelity of the trace capture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TraceIntegrity {
    pub ring_buffer_overflows: u64,
    pub events_dropped: u64,
    pub uprobe_attach_failures: Vec<String>,
    pub kprobe_attach_failures: Vec<String>,
    pub partial_captures: Vec<PartialCapture>,
    pub bloom_filter_capacity: u64,
    pub bloom_filter_false_positive_rate: f64,
}

/// Record of an event that was only partially captured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartialCapture {
    pub event_type: String,
    pub reason: String,
    pub timestamp: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_integrity_serde_round_trip() {
        let integrity = TraceIntegrity {
            ring_buffer_overflows: 0,
            events_dropped: 2,
            uprobe_attach_failures: vec!["libssl.so:SSL_write".to_string()],
            kprobe_attach_failures: vec![],
            partial_captures: vec![PartialCapture {
                event_type: "tls_handshake".to_string(),
                reason: "buffer too small".to_string(),
                timestamp: Timestamp::now(),
            }],
            bloom_filter_capacity: 100_000,
            bloom_filter_false_positive_rate: 0.01,
        };

        let json = serde_json::to_string(&integrity).expect("serialize integrity");
        let back: TraceIntegrity = serde_json::from_str(&json).expect("deserialize integrity");
        assert_eq!(integrity.events_dropped, back.events_dropped);
        assert_eq!(integrity.uprobe_attach_failures, back.uprobe_attach_failures);
    }

    /// Milestone 212 (issue #615) — wire-shape regression guard.
    ///
    /// Post-m212 `ring_buffer_overflows` carries real u64 drop counts
    /// (previously always `0`) and `kprobe_attach_failures[]` may
    /// carry counter-map names (e.g. `"file_event_drops"`) alongside
    /// real kprobe attach failure names per Q3. This test asserts:
    /// (a) the serialized JSON round-trips value-identically for BOTH
    ///     fields populated with realistic post-m212 values, AND
    /// (b) the deserialized struct is byte-equal to the input via
    ///     `serde_json::to_value` equality (JSON-value equivalence,
    ///     robust to serde version drift per research R4).
    #[test]
    fn trace_integrity_serde_populated_counter_and_attach_failures() {
        let integrity = TraceIntegrity {
            ring_buffer_overflows: 13636,
            events_dropped: 0,
            uprobe_attach_failures: vec![],
            kprobe_attach_failures: vec![
                "file_event_drops".to_string(),
                "vfs_open".to_string(),
            ],
            partial_captures: vec![],
            bloom_filter_capacity: 65536,
            bloom_filter_false_positive_rate: 0.01,
        };

        let json = serde_json::to_string(&integrity).expect("serialize integrity");
        let back: TraceIntegrity =
            serde_json::from_str(&json).expect("deserialize integrity");
        assert_eq!(integrity.ring_buffer_overflows, back.ring_buffer_overflows);
        assert_eq!(integrity.kprobe_attach_failures, back.kprobe_attach_failures);
        assert_eq!(integrity, back);

        // Cross-check via serde_json::Value equality — catches
        // structural drift (field renames, type changes) that
        // struct-equality can't.
        let val = serde_json::to_value(&integrity).unwrap();
        let val_back = serde_json::to_value(&back).unwrap();
        assert_eq!(val, val_back);

        // Field-presence assertions on the emitted JSON — pinned so
        // consumer contracts (per contracts/counter-semantics.md) can
        // rely on stable field names.
        assert!(val.get("ring_buffer_overflows").unwrap().is_u64());
        assert_eq!(val["ring_buffer_overflows"].as_u64(), Some(13636));
        assert!(val.get("kprobe_attach_failures").unwrap().is_array());
    }
}
