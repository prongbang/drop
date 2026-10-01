# Task 1: Server Signaling

## Objective and scope

Add participant-scoped signaling and direct-transfer completion to the existing in-memory transfer hub. The server carries offer/answer/ICE metadata, never file chunks on the direct path. Keep the existing relay endpoints working.

## Dependencies

None. Implements the signaling contract consumed by Task 02.

## Relevant files

- `src/drop.rs`
- `docs/superpowers/specs/2026-10-01-webrtc-fast-transfer-design.md`

## Steps

- [ ] Add a typed signaling message carrying transfer id, sender, recipient, message sequence/type, and serialized SDP or ICE payload. Keep messages bounded and only visible to the two transfer participants.
- [ ] Write and run failing Rust tests for valid participant exchange, rejection of nonparticipant signaling, and direct completion only by the receiver.
- [ ] Add hub methods and HTTP routes to enqueue/read/ack signaling and mark a direct transfer ready without storing file bytes. Preserve relay `upload` and `take` behavior.
- [ ] Run the focused Rust tests and `cargo check`; verify rejected signaling does not mutate transfer state.
- [ ] Commit the server signaling change.

## Acceptance criteria

- Only the sender and receiver can exchange signals for their transfer.
- A receiver can mark an accepted direct transfer complete; a bystander or sender cannot falsely mark it complete.
- Relay transfers remain available through existing routes.
- Signal storage is bounded by existing transfer lifetime/count behavior or an explicit per-transfer cap.
