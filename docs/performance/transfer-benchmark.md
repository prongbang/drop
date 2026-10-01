# Transfer Speed Baseline

Captured at commit `57aba07` (`fix(cli): download updates from published binaries`) as a local server-relay reference.

## Current server-relay path

| Measurement | Result |
| --- | ---: |
| Payload | 64 MiB (67,108,864 bytes) |
| Upload, curl clients over loopback | 0.017 s (4,040,025,525 B/s) |
| Download run 1 | 0.120 s (560,225,596 B/s) |
| Download run 2 | 0.134 s (501,250,076 B/s) |
| Download run 3 | 0.117 s (574,847,646 B/s) |
| Median download | 0.120 s (560,225,596 B/s, about 534 MiB/s) |

Environment: Apple M4 Pro, macOS 26.6.2, `bin/drop-darwin-arm64`, local server and curl clients on loopback. The sender uploaded the payload through the Drop HTTP API; the receiver fetched it through the transfer download route. The file was generated as random binary data. Download rate is payload bytes divided by elapsed wall time.

This is a reproducible local baseline, not a Wi-Fi or cross-device result. Loopback removes network latency and may make the relay's in-memory path look much faster than a real network transfer. For meaningful comparisons, use the same payload size, device arrangement, and timing boundaries.
