// Generated snapshot test scaffold for sample-source.
// Wire this into an adapter parser test once the adapter implementation is registered.

#[test]
fn sample_source_snapshot_scaffold_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(root.join("fixtures/sample-source/sample/bundle.json").exists());
    assert!(root.join("fixtures/sample-source/sample/snapshot.json").exists());
}
