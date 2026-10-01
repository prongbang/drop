# WebRTC Fast Transfer

**Goal:** Send accepted files directly between Drop browser clients when possible, retain the current HTTP relay as fallback, and compare speed against the recorded 64 MiB baseline.

**Constraints:** Keep transfer participation checks; direct-received files live in the open page session; do not require a third-party ICE service; preserve loopback-vs-LAN measurement caveats.

**Status:** Complete.

| Task | Status |
| --- | --- |
| [Task 01: Server signaling](done/task-01-server-signaling.md) | Done |
| [Task 02: Browser direct transfer](done/task-02-browser-direct-transfer.md) | Done |
| [Task 03: Performance comparison and release artifacts](done/task-03-performance-and-build.md) | Done |

Design: [WebRTC fast transfer spec](../../superpowers/specs/2026-10-01-webrtc-fast-transfer-design.md). Baseline: [transfer benchmark](../../performance/transfer-benchmark.md).
