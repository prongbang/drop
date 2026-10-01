# Transfer Speed Baseline

Captured before the WebRTC transfer implementation, from commit `57aba07` (`fix(cli): download updates from published binaries`).

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

This is a reproducible local baseline, not a Wi-Fi or cross-device result. Loopback removes network latency and may make the relay's in-memory path look much faster than a real device-to-device transfer. The comparison must retain this caveat and use the same payload size, device arrangement, and timing boundaries for both implementations. A separate two-device LAN measurement is needed to establish real-world improvement.

## Browser relay vs WebRTC direct

Captured after implementing direct browser transfer, using three 64 MiB transfers for each path. Each receiver downloaded the resulting file and checked every byte against the generated payload (67,108,864 bytes of `0x5a`); all six transfers had zero mismatches.

| Path | Run 1 | Run 2 | Run 3 | Median |
| --- | ---: | ---: | ---: | ---: |
| Browser server relay, end-to-end after Accept | 40.2 ms | 37.7 ms | 39.3 ms | 39.3 ms |
| WebRTC direct, end-to-end after Accept | 4,663.6 ms | 4,246.0 ms | 5,063.6 ms | 4,663.6 ms |
| WebRTC direct, displayed receive rate | 14 MB/s | 16 MB/s | 13 MB/s | 14 MB/s |

Environment: Apple M4 Pro, macOS 26.6.2, headless Chrome, two isolated browser contexts on the same machine, local Drop server on `localhost:8777`. End-to-end timing starts when the receiver accepts and ends when the received file appears. The browser relay measurement uses the HTTP-backed file list and includes the subsequent file fetch; the direct path uses the in-memory Blob link and includes the same fetch. WebRTC negotiated a `maxMessageSize` of 262,144 bytes. The HTTP baseline above used `curl` and has a different client/timing boundary, so it is provided as historical context rather than a directly comparable row.

On this same-machine topology, WebRTC is slower than the in-memory relay. This does not establish performance over Wi-Fi or between devices; it only confirms correctness and provides an initial browser comparison. Measure on a real LAN before claiming a device-to-device speedup.
