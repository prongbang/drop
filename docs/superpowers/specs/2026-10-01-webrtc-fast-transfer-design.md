# Faster Client-to-Client File Transfers

## Goal

Make files sent between Drop browser clients download faster by transferring bytes directly between peers when the browsers can establish a WebRTC connection. Preserve the existing HTTP relay as a compatibility fallback. Record measurements against the existing server-relay baseline before claiming a speed improvement.

## Current behavior

The browser creates a transfer offer through `POST /api/offer`. The receiver accepts with `POST /api/respond`. The sender then uploads the complete file to `POST /api/upload/{id}`; the Rust hub retains the `Bytes` payload in memory. The receiver downloads it from `GET /api/transfer/{id}`. The existing transfer cap is 100 MiB. Server-sent events publish peer presence and transfer state.

The baseline in [transfer-benchmark.md](../../performance/transfer-benchmark.md) measured a 64 MiB transfer on loopback. The server-relay download median was 0.120 s (560,225,596 B/s) across three runs on an Apple M4 Pro. This is a local synthetic result, not a LAN result.

## Proposed behavior

After the receiver accepts, the two browsers negotiate a peer connection using the existing Drop server only to exchange signaling messages. When the data channel opens, the sender transmits the file in bounded binary chunks, respecting channel backpressure. The receiver assembles the chunks into a `Blob` and exposes a normal download action in the existing received-files UI. The server does not handle file bytes on this successful path.

If WebRTC is unavailable, negotiation fails, or a direct connection cannot be established, clients use the current upload/download relay. No external STUN/TURN service is required for the initial implementation; same-LAN devices should be able to connect directly. Connections across network boundaries may require future configurable ICE servers and are expected to use the relay fallback for now.

The UI should show meaningful progress and a clear direct-versus-relayed status, while retaining existing accept/decline and transfer history behavior. Transfers continue to be private to their two participants. Signaling messages must be scoped to the offered transfer and authenticated by the existing per-client identity checks.

## Received-file lifetime decision

On the direct path, bytes exist in the receiving page as a `Blob`; unlike the current relay, the server has no copy to offer after a page reload. The simplest initial design keeps the received file available for download while that browser page remains open and falls back to relay when direct transfer is unavailable. The product behavior after reload needs approval: either accept this session-only lifetime, or persist direct-received blobs locally (for example, in IndexedDB), which adds storage quota, cleanup, and error-handling requirements.

## Performance comparison

Keep the old result unchanged as the baseline. Add a repeatable benchmark procedure for the new implementation using the same 64 MiB payload and report each run, median elapsed time, throughput, device topology, browser, OS, and whether the path was direct or relayed. First compare in a controlled local setup; then measure two actual devices on the same LAN if available. Do not represent localhost measurements as Wi-Fi performance, and do not claim a speedup unless equivalent measurements show one.

## Acceptance criteria

- Compatible browsers complete a direct peer transfer without sending the payload through the Rust server.
- The existing relay still succeeds when direct transfer is unsupported or negotiation fails.
- Receiver can download the completed direct transfer and sees accurate progress and transfer status.
- Existing accept/decline flow and participant privacy remain intact.
- The 64 MiB performance comparison reports raw runs, median, throughput, and topology against the preserved baseline.
- Embedded web assets and supported project binaries are rebuilt for a deliverable.

## Out of scope

- Cloud relay service, public signaling service, or mandatory third-party STUN/TURN dependency.
- Removing the existing relay path or its current transfer-size behavior.
- Claiming real-device network performance from a loopback benchmark.
