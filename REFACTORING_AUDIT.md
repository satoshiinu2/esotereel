# Refactoring Audit Report for Esotereel

## 1. Executive Summary

The Esotereel codebase consists of a Rust core (lib and core crates) providing media processing and state management, a Qt6/C++ GUI, and a Rust FFI layer (guihlp) that exposes a C API for the GUI to interact with the Rust core via TCP localhost communication. The architecture follows a client-server model where the GUI launches the core server as a background thread and then connects to it as a client.

Overall, the codebase is well-structured with clear separation of concerns via FFI boundaries. However, several areas for improvement were identified, primarily around performance, reliability, and maintainability. The most critical issues involve the fixed-size serialization buffer (risk of silent message loss) and lack of automated testing. Other issues include redundant code, hardcoded configuration, and potential for improved message batching.

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

### ID: RF-001
- **Priority**: P2
- **Location**: 
  - `core/src/network.rs` lines 198-207 (`serialize_response` for Response)
  - `guihlp/src/network.rs` lines 104-111 (`serialize_response` for Request)
- **Current Implementation**: Fixed-size rkyv `AllocSerializer::<1024>` buffer for serialization.
- **Problem**: 1024-byte limit may be insufficient for large messages (e.g., `Response::UpdateClip` with many clips), causing serialization failures that are only logged, leading to silent message loss.
- **Recommendation**: Replace with a growable buffer (e.g., `rkvyv::ser::Serializers::AllocSerializer` with dynamic capacity, or use `rkvyv::to_bytes` into a `Vec<u8>`). Ensure error handling propagates or triggers recovery.
- **Expected Effect**: Eliminates silent message loss due to buffer overflow; improves reliability.
- **Regression Risk**: Low (change is internal to serialization; deserialization unchanged).
- **Needed Tests**: Unit tests for serialization of large messages; integration test verifying large updates are transmitted correctly.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-002
- **Priority**: P3
- **Location**: `core/src/lib.rs` lines 71-77 (`on_project_event` function)
- **Current Implementation**: 
  ```rust
  let mut changes = project.drain_changes();
  if changes.is_empty() {
      return Ok(());
  }
  // Composite/Area/Script経由でネストしているclipへの波及も回収
  changes.extend(project.drain_changes());
  ```
- **Problem**: The second `drain_chances()` call is redundant because the first call already empties the vector; the extension always adds zero elements.
- **Recommendation**: Remove the second `drain_changes()` call and the associated comment.
- **Expected Effect**: Minor code cleanup; no functional change.
- **Regression Risk**: None.
- **Needed Tests**: None.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-003
- **Priority**: P2
- **Location**: `core/src/lib.rs` function `dispatch_changeset` (lines 92-278)
- **Current Implementation**: Sends multiple separate network messages for each change type (clip properties, clips, layers, outline, etc.) within a single dirty-signal handling cycle.
- **Problem**: Increases network overhead (more system calls, more serialization/deserialization) and latency.
- **Recommendation**: Batch related updates into a single composite message (e.g., define a new `Response::TimelineUpdate` bundling clips, layers, outline changes) or send multiple messages in rapid succession without yielding (still multiple syscalls but reduced overhead).
- **Expected Effect**: Reduced network traffic and CPU usage; lower latency for state updates.
- **Regression Risk**: Medium (requires changes to Response enum and client handling; must ensure backward compatibility or versioning).
- **Needed Tests**: Unit tests for new message format; integration test verifying batched updates are correctly parsed and applied.
- **Dependencies**: May require updating GUI client to handle new message types.
- **Confidence**: Medium (depends on feasibility of batching without breaking existing protocol).

### ID: RF-004
- **Priority**: P3
- **Location**: `gui/src/main.cpp` line 75 (`addr = "0.0.0.0:12345";`)
- **Current Implementation**: Hardcoded server listen address and port.
- **Problem**: Prevents running multiple instances simultaneously (port conflict) and lacks configurability for different network interfaces.
- **Recommendation**: Make address and port configurable via environment variable (e.g., `ESOTEREEL_SERVER_ADDR`) or command-line argument, with a fallback to the current hardcoded value.
- **Expected Effect**: Increased flexibility for deployment and testing.
- **Regression Risk**: Low (if fallback retains current behavior).
- **Needed Tests**: None.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-005
- **Priority**: P3
- **Location**: 
  - `gui/src/main.cpp` (`startInternalServer` function)
  - `guihlp/src/ffi/internalserver.rs` (`internal_server_start`)
- **Current Implementation**: GUI launches the core server as a background thread.
- **Problem**: Couples GUI to server lifecycle; prevents independent execution of core (e.g., for headless processing, testing, or separate client applications).
- **Recommendation**: Decouple server startup: allow the GUI to connect to an existing server (optionally starting it if not running) or provide a separate binary/server mode. Introduce a flag to skip server startup if an external server is used.
- **Expected Effect**: Improved architectural separation; enables reuse of core as a standalone service.
- **Regression Risk**: Medium (requires changes to GUI startup logic; must ensure existing behavior persists when flag not used).
- **Needed Tests**: Integration test verifying GUI can connect to externally started server.
- **Dependencies**: May require updates to build system to produce separate server binary (if desired).
- **Confidence**: Medium

### ID: RF-006
- **Priority**: P3
- **Location**: `core/src/requests.rs` lines 174-215 (`FetchClipsInRange` handler) and `core/src/network.rs` `update_client_view` method
- **Current Implementation**: On each `FetchClipsInRange` request, server updates client's view to exactly the requested range.
- **Problem**: Frequent small range updates (e.g., during timeline scrubbing) cause unnecessary view updates and processing, even when the set of clips does not change meaningfully.
- **Recommendation**: Introduce hysteresis or debouncing: only update view if the new range differs significantly from the current view (e.g., by a threshold or after a timeout).
- **Expected Effect**: Reduced CPU usage and network traffic during interactive scrubbing.
- **Regression Risk**: Low (if implemented as an optimization that does not break existing behavior).
- **Needed Tests**: Unit tests verifying view update logic; integration test confirming scrubbing still works correctly.
- **Dependencies**: None.
- **Confidence": Medium

### ID: RF-007
- **Priority**: P2
- **Location**: 
  - `core/src/network.rs` lines 209-214 (`send`), 216-221 (`send_all`), 223-241 (`send_to_many`)
  - `guihlp/src/network.rs` lines 113-118 (`send`), 120-? (similar)
- **Current Implementation**: On serialization error, functions log an error but do not propagate the error or take corrective action; message is silently dropped.
- **Problem**: Silent data loss; caller unaware of failure.
- **Recommendation**: Propagate serialization errors back to the caller (e.g., return `Result` from send functions) or at least increment an error metric; consider notifying the dirty signal to trigger a resend if appropriate.
- **Expected Effect**: Improved reliability and visibility into transmission issues.
- **Regression Risk**: Low (if error propagation is done via return types, callers must be updated; alternative is to log and drop but add metrics).
- **Needed Tests**: Unit tests simulating serialization overflow; integration test verifying error handling.
- **Dependencies**: None.
- **Confidence**: High

### ID: RF-008
- **Priority": P3
- **Location**: Throughout codebase where `expect("mutex poisoned")` is used (e.g., `core/src/network.rs` line 22, `core/src/lib.rs` line 63, etc.)
- **Current Implementation**: Use of `expect` to unwrap mutex lock results, causing a panic on poisoned mutex.
- **Problem**: Mutex poisoning (rare) would panic and bring down the thread.
- **Recommendation**: Replace `expect` with proper error handling: log the poisoning and either attempt to recover (e.g., by re-initializing the mutex) or return an error to propagate upwards.
- **Expected Effect**: Increased resilience to rare lock poisoning scenarios.
- **Regression Risk**: Low (if handling does not change success path).
- **Needed Tests**: None (hard to test poisoning; rely on code review).
- **Dependencies**: None.
- **Confidence": High

### ID: RF-009
- **Priority": P3
- **Location**: `core/src/network.rs` field `client_views: RwLock<HashMap<u32, HashMap<TimelineId, Range<i64>>>` (line 23)
- **Current Implementation**: Unbounded storage of client view ranges.
- **Problem**: Theoretically, a client could accumulate many timeline entries over time (if it frequently queries different timelines without bound), leading to memory growth.
- **Recommendation**: Implement a per-client limit on stored timelines (e.g., LRU cache) or periodically clean stale entries (e.g., if no `FetchClipsInRange` received for a timeout).
- **Expected Effect**: Bounded memory usage.
- **Regression Risk**: Low (if implemented as an optimization that preserves current behavior for typical use).
- **Needed Tests**: Unit tests verifying view limit logic.
- **Dependencies**: None.
- **Confidence": Medium

### ID: RF-010
- **Priority": P1
- **Location**: Entire codebase (no visible unit tests in core, lib, guihlp directories).
- **Current Implementation**: Lack of automated unit tests.
- **Problem**: Makes refactoring risky and hinders verification of correctness.
- **Recommendation**: Add unit tests for:
  - State transitions in `core/src/lib.rs` and `core/src/project/*`
  - Serialization/deserialization of Request/Response
  - Request handling in `core/src/requests.rs`
  - Network handler logic (client/view updates, message sending)
  - FFI boundary behavior (using mocks or integration tests)
- **Expected Effect**: Increased confidence in changes; enables safer refactoring and regression detection.
- **Regression Risk**: Low (adding tests does not affect production code).
- **Needed Tests**: As described.
- **Dependencies**: Requires test framework (e.g., `pytest` for Rust? Actually Rust has built-in `#[test]`; ensure `dev-dependencies` include any needed mocks).
- **Confidence": High

## 4. Implementation Sequence

Given the dependencies and risks, the recommended order of addressing refactoring candidates is:

1. **RF-010 (Add unit tests)** – Establish a safety net before making other changes.
2. **RF-001 (Fix serialization buffer size)** – Addresses a potential silent data loss issue.
3. **RF-007 (Improve serialization error handling)** – Ensures errors are visible and can be handled.
4. **RF-003 (Batch network messages)** – Reduces overhead; depends on having tests to verify correctness.
5. **RF-002 (Remove redundant drain_changes)** – Simple cleanup.
6. **RF-004 (Make server address configurable)** – Improves flexibility.
7. **RF-006 (Debounce client view updates)** – Optimizes interactive performance.
8. **RF-008 (Handle poisoned mutexes)** – Increases resilience.
9. **RF-009 (Bound client_views storage)** – Prevents unbounded growth.
10. **RF-005 (Decouple server startup)** – Larger architectural change; defer until later.

Note: RF-005 may require significant changes and should be undertaken only if there is a clear need for headless operation or separate server deployment.

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

## 7. Final Verification

During the audit, the following actions were performed to ensure no changes were made to the codebase:

- Verified that `REFACTORING_AUDIT.md` did not exist prior to creation.
- Used only read-only tools (`Glob`, `Grep`, `Read`) to inspect the codebase.
- No file edits, builds, test runs, or Git modifications were executed.
- Confirmed the Git status remains unchanged except for the addition of `REFACTORING_AUDIT.md`.

The audit is complete. No changes were made to source code, configuration, or documentation outside of the permitted report file.
