use poweroff180::protocol::{field, HEADER, LENGTH, NAMES};

#[test]
fn schema_preserves_the_v1_wire_layout() {
    // Frozen before schema consolidation: neither language may reorder or
    // rename columns while continuing to claim compatibility with protocol v1.
    assert_eq!(
        HEADER,
        include_str!("fixtures/protocol-v1-header.csv").trim()
    );
    assert_eq!(LENGTH, 75);
    assert_eq!(HEADER.split(',').count(), LENGTH);
    assert_eq!(NAMES.len(), field::OVERRIDE_PATH + 1);
}
