# Refactoring Audit Report for Esotereel

> **Status update (2026-11-19)**: This audit was reviewed against master
> `8f511c4`. Two of its headline findings did not survive that review:
> RF-001 rested on a wrong premise about rkyv and is **withdrawn**, and
> RF-010's premise (no automated tests) no longer holds and is also
> **withdrawn**. RF-008 is **resolved**. The remaining candidates were
> re-checked, their locations updated, and one new candidate (RF-011)
> was added for the test contracts that are still unfixed. Withdrawn and
> resolved items are kept below, clearly marked, so the history of the
> audit stays readable.

## 1. Executive Summary

The Esotereel codebase consists of a Rust core (lib and core crates) providing media processing and state management, a Qt6/C++ GUI, and a Rust FFI layer (guihlp) that exposes a C API for the GUI to interact with the Rust core via TCP localhost communication. The architecture follows a client-server model where the GUI launches the core server as a background thread and then connects to it as a client.

Overall, the codebase is well-structured with clear separation of concerns via FFI boundaries. The remaining areas for improvement are around reliability and maintainability: a redundant (and semantically ineffective) change-drain in the event path, unbatched network updates, hardcoded server addresses, send paths that drop failures silently, and a client-view cleanup that is defined but never called. The previously reported "most critical" issues — a fixed-size serialization buffer risking silent message loss, and a lack of automated testing — were re-examined and withdrawn (see RF-001 and RF-010): the serialization output was never size-capped, and the test suite now exists and runs in CI.

No safety-critical or data corruption issues were found, but addressing the identified refactoring candidates would enhance robustness, performance, and future maintainability.

## 2. Architecture Overview

### Modules and Responsibilities

- **esotereel_lib**: Core library providing decoding, plugin management, project data structures, rendering utilities, and network trait definitions.
- **esotereel_core**: Server-side logic that maintains authoritative project state, handles client requests, and broadcasts updates via TCP.
- **esotereel_gui_helper**: FFI layer that provides a C API for the GUI, includes the client network handler, and generates C headers via cbindgen.
- **gui/src**: Qt6/C++ GUI that uses the FFI to start the server, connect as a client, send requests, and render UI based on state updates.

### Data Flow

1. GUI starts (`main.cpp`):
   - Creates `ClientState` (Rust struct via FFI).
   - Launches core server in a background thread via `InternalServer::start` → `guihlp::internal_server_start` → spawns thread running `core::server_network_start`.
   - After server start callback, launches client network handler via `state->run` → `guihlp::client_state_network_run` → spawns thread running `ClientNetworkHandler::run`.

2. Core server listens on TCP port 12345 (hardcoded) for client connections.

3. GUI client connects to the server, performs handshake, and enters message exchange loop:
   - Sends Requests (e.g., NewProject, Command, FetchClipsInRange) serialized with rkyv.
   - Receives Responses (e.g., UpdateClip, ProjectMeta) and updates internal state.
   - Server processes requests, modifies authoritative state, and notifies listeners via a dirty signal.
   - On dirty signal, server scans state changes and sends relevant Updates to subscribed clients based on their viewed timelines and ranges.

4. GUI receives updates, marks timelines dirty, and triggers UI refresh via callbacks (e.g., `mark_dirty_timeline`).

### Dependencies

- GUI → (via FFI) → esotereel_gui_helper → (depends on) esotereel_lib, esotereel_core
- esotereel_core → depends on esotereel_lib
- No circular dependencies.

## 3. Refactoring Candidates

### ID: RF-001 — Withdrawn (incorrect premise)

- **Status**: Withdrawn on 2026-11-19. Do not implement the original recommendation.
- **Original claim**: The fixed-size rkyv `AllocSerializer::<1024>` buffer caps messages at 1024 bytes, so large messages (e.g., `Response::UpdateClip` with many clips) fail serialization and are silently lost.
- **Why it was wrong**: In rkyv 0.7.46 (the version in `Cargo.lock`), `AllocSerializer<N>` is a `CompositeSerializer<AlignedSerializer<AlignedVec>, FallbackScratch<HeapScratch<N>, AllocScratch>, SharedSerializeMap>`. The output goes to an `AlignedVec`, which grows; `N` (here 1024) is only the amount of scratch space pre-allocated before allocations spill into heap memory. There is no 1024-byte limit on message size, and scratch exhaustion does not fail serialization.
- **What remains**: The serializer calls still exist (`core/src/network.rs` line 204, `guihlp/src/network.rs` line 105) and need no change. The separate question of what happens when serialization *does* fail for other reasons is RF-007, re-scoped there.

### ID: RF-002

- **Priority**: P3
- **Location**: `core/src/lib.rs` lines 73-79 (`on_project_event` function, starting at line 61)
- **Current Implementation**:
  ```rust
  let mut changes = project.drain_changes();
  if changes.is_empty() {
      return Ok(());
  }
  // Composite/Area/Script経由でネストしているclipへの波及も回収
  changes.extend(project.drain_changes());
  ```
- **Problem**: The second `drain_changes()` call is redundant because the first call already empties the vector; the extension always adds zero elements. There is also a semantic problem the original audit did not note: even if the second call returned entries, `extend` on `Vec<(TimelineId, ChangeSet)>` would duplicate entries per timeline instead of merging them, and the caller dispatches each entry separately — so the intended "collect nested propagation" would become a duplicate dispatch, not a merge. Merging would require `ChangeSet::merge` per `TimelineId`.
- **Recommendation**: Either remove the second `drain_changes()` call and the associated comment (after confirming nested propagation is covered elsewhere), or replace the `extend` with a per-timeline merge via `ChangeSet::merge` so the stated intent actually works. Pick one explicitly; do not leave the current form.
- **Expected Effect**: Minor code cleanup; no functional change if removed.
- **Regression Risk**: None for removal.
- **Needed Tests**: None for removal. A merge implementation should be covered by the dispatch contract tests proposed in RF-011.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-003

- **Priority**: P2
- **Location**: `core/src/lib.rs` function `dispatch_changeset` (line 94 onwards)
- **Current Implementation**: Sends multiple separate network messages for each change type (clip properties, clips, layers, outline, etc.) within a single dirty-signal handling cycle. Clip property changes are sent one message per changed property, so the actual splitting is finer than the original audit described.
- **Problem**: Increases network overhead (more system calls, more serialization/deserialization) and latency.
- **Recommendation**: Batch related updates into a single composite message (e.g., define a new `Response::TimelineUpdate` bundling clips, layers, outline changes) or send multiple messages in rapid succession without yielding (still multiple syscalls but reduced overhead).
- **Expected Effect**: Reduced network traffic and CPU usage; lower latency for state updates.
- **Regression Risk**: Medium (requires changes to Response enum and client handling; must ensure backward compatibility or versioning).
- **Needed Tests**: Unit tests for new message format; integration test verifying batched updates are correctly parsed and applied.
- **Dependencies**: May require updating GUI client to handle new message types.
- **Confidence**: Medium (depends on feasibility of batching without breaking existing protocol).

### ID: RF-004

- **Priority**: P3
- **Location**:
  - `gui/src/main.cpp` line 75 (`addr = "127.0.0.1:12345";`)
  - `core/src/main.rs` line 28 (`server_network_start("0.0.0.0:12345", ...)`, the standalone server binary — not covered by the original audit)
- **Current Implementation**: Hardcoded server listen address and port in both entry points. Note the asymmetry: the GUI path binds localhost only, while the standalone binary binds all interfaces (`0.0.0.0`).
- **Problem**: Prevents running multiple instances simultaneously (port conflict) and lacks configurability for different network interfaces.
- **Recommendation**: Make address and port configurable via environment variable (e.g., `ESOTEREEL_SERVER_ADDR`) or command-line argument, with a fallback to the current hardcoded values. Cover both locations.
- **Expected Effect**: Increased flexibility for deployment and testing.
- **Regression Risk**: Low (if fallback retains current behavior).
- **Needed Tests**: None.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-005 — Partially resolved

- **Priority**: P3
- **Status**: The server side is already decoupled; only the GUI side remains.
- **Location**:
  - `gui/src/main.cpp` (`startInternalServer` function, called unconditionally)
  - `guihlp/src/ffi/internalserver.rs` (`internal_server_start`)
- **Current Implementation**: GUI launches the core server as a background thread. However, a standalone server binary already exists: `core/src/main.rs` runs `server_network_start` directly, CMake builds it as the separate `rust_core` target outside the GUI's `ALL`, and `scripts/smoke-test.sh` exercises it as an external server.
- **Problem**: The GUI has no way to connect to an already-running server or to skip starting its own, so the GUI is still coupled to the server lifecycle even though the core is not.
- **Recommendation**: Allow the GUI to connect to an existing server (optionally starting it if not running). Introduce a flag to skip server startup if an external server is used. Do not re-implement the standalone binary; it exists.
- **Expected Effect**: Improved architectural separation; enables reuse of core as a standalone service from the GUI as well.
- **Regression Risk**: Medium (requires changes to GUI startup logic; must ensure existing behavior persists when flag not used).
- **Needed Tests**: Integration test verifying GUI can connect to externally started server.
- **Dependencies**: None for the server binary (already built separately).
- **Confidence**: Medium

### ID: RF-006 — Partially resolved

- **Priority**: P3
- **Location**: `core/src/requests.rs` `FetchClipsInRange` handler (line 479 onwards; the `update_client_view` call is at line 502) and `core/src/network.rs` `update_client_view` method (line 150)
- **Current Implementation**: On each `FetchClipsInRange` request, the server updates the client's view to exactly the requested range, with no threshold or debounce. The client side, however, already suppresses exact duplicates: `guihlp/src/ffi/requests.rs` (`req_fetch_frame`) compares against `last_clips_fetch` and does not resend a `FetchClipsInRange` for an identical timeline and range.
- **Problem**: Narrower than originally described: identical-range repeats are already filtered on the client. What remains is frequent *slightly different* range updates (e.g., during timeline scrubbing) causing unnecessary server-side view updates and processing.
- **Recommendation**: Introduce hysteresis on the remaining path: only update the server-side view if the new range differs significantly from the current view (e.g., by a threshold or after a timeout).
- **Expected Effect**: Reduced CPU usage and network traffic during interactive scrubbing.
- **Regression Risk**: Low (if implemented as an optimization that does not break existing behavior).
- **Needed Tests**: Unit tests verifying view update logic; integration test confirming scrubbing still works correctly.
- **Dependencies**: None.
- **Confidence**: Medium

### ID: RF-007

- **Priority**: P3 (lowered from P2; see Problem)
- **Location**:
  - `core/src/network.rs` lines 213 (`send`), 220 (`send_all`), 227 (`send_to_many`)
  - `guihlp/src/network.rs` lines 112 (`send`), 119 (`send_bytes`)
- **Current Implementation**: On serialization error, functions log an error but do not propagate the error or take corrective action; the message is silently dropped. The send functions return `()`.
- **Problem**: Silent data loss; caller unaware of failure. This is now scoped independently of RF-001: with the buffer-overflow scenario withdrawn, the realistic failure source is a closed channel (sending to a disconnected client), which is a consequence of disconnection rather than of message size. That is why the priority is lowered.
- **Recommendation**: Propagate errors back to the caller (e.g., return `Result` from send functions) or at least increment an error metric; consider notifying the dirty signal to trigger a resend if appropriate.
- **Expected Effect**: Improved reliability and visibility into transmission issues.
- **Regression Risk**: Low (if error propagation is done via return types, callers must be updated; alternative is to log and drop but add metrics).
- **Needed Tests**: Tests for the round-trip contract in RF-011 first, so error-path changes are verifiable; integration test verifying error handling.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-008 — Resolved

- **Status**: Resolved. `expect("mutex poisoned")` no longer exists anywhere in the codebase (0 occurrences).
- **Resolution**: `lib/src/util/result.rs` (lines 110-156) provides `LockExt` (for `RwLock`) and `MutexExt` (for `Mutex`). On poisoning they recover the guard via `into_inner`, log the recovery with `log::error!`, and let the operation continue, instead of panicking. The remaining `expect()` calls in production code cover unrecoverable invariants only (GPU adapter acquisition, Tokio runtime creation, embedded settings schemas), which the project rules in `AGENTS.md` explicitly allow.

### ID: RF-009

- **Priority**: P3
- **Location**: `core/src/network.rs` field `client_views: RwLock<HashMap<u32, HashMap<TimelineId, Range<i64>>>>` (line 24); cleanup function `remove_client_view` (line 191)
- **Current Implementation**: Unbounded storage of client view ranges, with no limit, LRU, or timeout cleanup.
- **Problem**: More concrete than the original audit stated. `remove_client_view` is defined but has **zero call sites**: the disconnect cleanup removes the client from `clients` but never touches `client_views`, so the views of every disconnected client remain for the lifetime of the process. The theoretical accumulation within one long-lived client also remains, but the disconnected-client leak is deterministic, not theoretical.
- **Recommendation**: First, call `remove_client_view` from the disconnect cleanup path — that is the primary fix. Only after that, consider a per-client limit on stored timelines (e.g., LRU cache) or periodic cleanup of stale entries.
- **Expected Effect**: Bounded memory usage; disconnected clients no longer leak view state.
- **Regression Risk**: Low.
- **Needed Tests**: A test for the view contract in RF-011 (connect → disconnect → no residual views).
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-010 — Withdrawn (premise no longer holds)

- **Status**: Withdrawn on 2026-11-19. The original claim ("no visible unit tests", Priority P1) is the opposite of the current state.
- **Current state**: Tests live under each crate's `tests/unit/` directory (the `src` side only carries `#[cfg(test)]` + `#[path]` declarations). As of master `8f511c4` there are 125 tests in total: lib 116, core 7, guihlp 2. Five of the lib tests are `#[ignore]`d because they need a wgpu adapter or a real media file (`ESOTEREEL_TEST_VIDEO`); the normal run is therefore lib 111 / core 7 / guihlp 2. CI runs `cargo test --workspace --release`, and `scripts/smoke-test.sh` plus `core/examples/smoke_client.rs` provide a server round-trip smoke check.
- **What remains**: Not "add tests" in general. The specific contracts that are still unfixed are listed as RF-011, scoped by the project's testing policy (verify observable behavior and contracts, not counts or coverage).

### ID: RF-011

- **Priority**: P2
- **Location**:
  - `core/src/network.rs` (serialization → send round trip)
  - `core/src/lib.rs` (`on_project_event` / `dispatch_changeset`)
  - `core/src/network.rs` + `core/src/requests.rs` (client view lifecycle)
- **Current Implementation**: The areas above have no tests fixing their observable contracts, while neighboring areas (plugin components/properties/shaders, project clips/assets, render requests) are already covered.
- **Problem**: RF-002, RF-003, RF-006, RF-007 and RF-009 all modify behavior in exactly these areas. Without the contracts fixed first, a regression introduced by those changes would not be observable.
- **Recommendation**: Add tests only for these three contracts:
  1. **Network round trip**: a `Response` serialized by `serialize_response` and sent via `send` / `send_to_many` can be read back on the other side (`check_archived_root::<Request/Response>`). This is about the round-trip contract, not message size (see RF-001).
  2. **Change → dispatch correspondence**: which network messages a given `ChangeSet` produces from `on_project_event` / `dispatch_changeset`, so RF-002 and RF-003 can be changed safely.
  3. **View lifecycle**: `FetchClipsInRange` → `update_client_view` → `clients_watching_in`, including that disconnecting a client leaves no residual views (RF-009).
- **Expected Effect**: The remaining refactoring candidates become safely verifiable.
- **Regression Risk**: Low (adding tests does not affect production code).
- **Dependencies**: None.
- **Confidence**: High

## 4. Implementation Sequence

Given the dependencies and risks, the recommended order of addressing the remaining refactoring candidates is:

1. **RF-011 (Fix the three unfixed contracts)** – Establish a safety net for the specific areas the other items touch.
2. **RF-009 (Connect `remove_client_view` to disconnect cleanup)** – Small, deterministic leak fix.
3. **RF-002 (Resolve the redundant drain_changes)** – Simple cleanup or merge fix.
4. **RF-007 (Improve send error handling)** – Ensures failures are visible and can be handled.
5. **RF-003 (Batch network messages)** – Reduces overhead; depends on having tests to verify correctness.
6. **RF-004 (Make server addresses configurable)** – Improves flexibility; covers both hardcoded locations.
7. **RF-006 (Hysteresis for near-range view updates)** – Optimizes the remaining scrubbing path.
8. **RF-005 (GUI option to use an external server)** – Only the GUI half remains; the standalone server already exists.

RF-001 and RF-010 are withdrawn and RF-008 is resolved; none of them appear in this sequence.

## 5. Changes to Avoid

- **Major refactoring of the FFI boundary**: The current FFI design is clean and sufficient; rewriting it would introduce risk without clear benefit.
- **Changing the communication protocol to something other than TCP localhost**: The current choice provides isolation and potential for remote clients; switching to shared memory or direct function calls would reduce flexibility.
- **Removing the dirty signal mechanism**: This is a key part of the update propagation; replacing it with polling would increase CPU usage.
- **Altering the core data structures (e.g., replacing rkyv with another serialization format)**: rkyv provides zero-deserialization overhead; changing it would likely increase CPU usage unless profiling shows a bottleneck.

## 6. Unconfirmed Items

- **Performance under load**: Without benchmarks, it is unclear whether the current serialization batching or network frequency is a bottleneck in practice. Profiling would be needed to confirm performance hypotheses.
- **Memory usage of large projects**: The project data structures (in lib) may have scaling issues with very large numbers of clips/layers; this would require testing with realistic media files.
- **Thread safety of plugin callbacks**: The FFI allows plugins to invoke callbacks into the GUI; ensuring these callbacks are thread-safe (e.g., via Qt's queued connections) is assumed but not verified in the audit.
- **Cross-platform networking behavior**: The code uses Tokio TCP; behavior on non-Windows platforms (Linux/macOS) is assumed to be similar but not verified.
