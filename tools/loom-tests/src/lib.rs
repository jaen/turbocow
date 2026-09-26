// Loom concurrency tests for turbocow's custom reference counting.
//
// This is a separate crate so that `--cfg loom` doesn't propagate to
// dev-dependencies (ecow, lean_string, …) that break under loom.
//
// Run with:
//   cd tools/loom-tests && RUSTFLAGS="--cfg loom" cargo test --release
