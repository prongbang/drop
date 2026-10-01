# Task 3: Performance Comparison and Build Artifacts

## Objective and scope

Measure the implemented direct and relay paths against the preserved baseline under a documented topology, then rebuild embedded web assets and project binaries.

## Dependencies

- Task 02 direct transfer and fallback complete.

## Relevant files

- `docs/performance/transfer-benchmark.md`
- `Makefile`
- `build.rs`
- `web/build/` (generated)
- `bin/` (generated binaries)

## Steps

- [x] Run three 64 MiB direct and relay transfers and record raw elapsed times, median, browser/OS, topology, and selected transport.
- [x] Compare the relay and direct path; document that same-machine localhost is not a real LAN result and that direct is slower in this topology.
- [x] Update the benchmark report with measured values and retain the original baseline unchanged.
- [x] Rebuild embedded web assets and both supported macOS binaries. `build.rs` embeds `web/build`; the final release builds completed after `bun run build`.
- [x] Commit benchmark and build artifacts as appropriate; push the completed feature branch after all implementation and verification are complete.

## Acceptance criteria

- The report includes every run, median, throughput, payload size, and topology for direct and relay results.
- Loopback performance is clearly labeled and never presented as Wi-Fi performance.
- Web assets and supported binaries correspond to the final source revision.
- No speedup is claimed unless measurements support it.

## Results

- Browser relay end-to-end runs: 40.2, 37.7, 39.3 ms (median 39.3 ms).
- WebRTC direct end-to-end runs: 4,663.6, 4,246.0, 5,063.6 ms (median 4,663.6 ms); all 64 MiB files passed full byte validation.
- `bun run build` completed and generated the embedded static web assets.
- `CARGO_TARGET_DIR=/Users/inteniquetic/.cargo-target make build_binaries` built `bin/drop-darwin-x86_64` and `bin/drop-darwin-arm64` successfully.
