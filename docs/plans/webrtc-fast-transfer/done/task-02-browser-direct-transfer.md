# Task 2: Browser Direct Transfer

## Objective and scope

Use WebRTC DataChannel for accepted browser-to-browser files with reliable ordered binary chunks, backpressure, progress, a downloadable page-session Blob, and automatic fallback to the existing HTTP relay.

## Dependencies

- Task 01 server signaling routes and completion behavior.

## Relevant files

- `web/src/routes/+page.svelte`
- `web/src/lib/transfer.ts` (create if it keeps transport logic out of the page)
- `web/package.json`
- `docs/superpowers/specs/2026-10-01-webrtc-fast-transfer-design.md`

## Steps

- [x] Write focused tests for chunk boundaries/backpressure decisions and direct-vs-relay transfer state using Bun's built-in test runner; run them and confirm they fail for the missing behavior.
- [x] Add the smallest transfer helper that chunks a `File`, observes DataChannel buffered amount, accepts chunks into a receiver Blob, and reports progress.
- [x] Implement sender-initiated WebRTC negotiation over Task 01 signaling. Use reliable ordered DataChannel messages and explicit transfer id/length metadata; handle ICE candidates and connection timeout.
- [x] After acceptance, attempt the direct path. If the browser lacks WebRTC or negotiation/channel setup fails, upload through the existing relay route.
- [x] On direct completion, create a page-session object URL, notify the server completion route, show direct/relay status and progress, and revoke URLs when the page unloads.
- [x] Exercise two browser clients through the actual UI: accept, direct 64 MiB transfer with byte-for-byte integrity verification, and relay fallback. Run `bun run check` and `bun run build`.
- [x] Commit the browser transfer change as part of the completed feature branch.

## Acceptance criteria

- A supported same-LAN browser pair transfers file bytes without posting them to the server.
- Receiver can download an intact direct transfer while its page remains open.
- Unsupported/failed direct transfer falls back automatically and still completes over HTTP relay.
- Progress and transport status are clear; accept and decline continue to work.
- Oversized files, disconnects, zero-byte files, and duplicate/stale signaling fail or recover without falsely showing a completed file.

## Verification notes

- `bun test`: 4 passing tests; `bun run check`: zero Svelte errors/warnings.
- Two isolated headless Chrome clients completed three 64 MiB direct transfers and three 64 MiB relay transfers; all 402,653,184 received bytes matched the generated data.
- Relay fallback was exercised by disabling WebRTC in the sender client. Direct mode produced Blob download links and relay mode produced server API links.
- The UI test covered Accept and successful receive, plus automatic relay fallback. Decline and mid-transfer disconnect were not separately exercised in the browser.
