# Issues Found in Esotereel Implementation

## 1. FFI Boundary Issues

### 1.1 Unnecessary Lifetime Transmute in `client_state_project_lock_read`
**File:** `guihlp/src/ffi/state.rs` (lines 214-219)

The function uses `std::mem::transmute` to extend the lifetime of a `RwLockReadGuard<'_, Option<Project>>` to `'static`. This is unnecessary and misleading because:
- The guard's actual lifetime is tied to the `Box<ProjectReadGuardInner>` that owns it, not to `'static`.
- While the lock lives for the entire program's lifetime (making the transmute technically safe in this specific context), it violates the principle clear ownership and lifetimes (AGENTS.md: "FFIを介したデータ渡しでは、所有権とライフタイムに特に注意する").
- It could lead to confusion if the lock's lifetime ever changes (e.g., if the locking strategy is modified).

**Recommendation:** Remove the transmute and rely on the `Box` to manage the guard's lifetime. The unlock function already correctly drops the `Box`, which will drop the guard and release the lock.

### 1.2 Potential Deadlock in Project Locking Order
**File:** `core/src/lib.rs` (functions `on_project_event` and `dispatch_changeset`)

The `on_project_event` function holds a write lock on the project (`state_guard.project.write()`) while calling `dispatch_changeset`, which attempts to acquire a write lock on the same project. This creates a deadlock scenario:
- Same-thread deadlock: The thread already holds the write lock and tries to acquire it again (non-reentrant lock).
- Cross-thread deadlock risk: If another thread attempts to acquire the project lock (for read or write) while holding the state lock, it could deadlock with a thread holding the project lock and waiting for the state lock.

**Recommendation:** Extract all necessary data from the project and timeline while holding the project lock, then release the lock before calling `dispatch_changeset`. The `dispatch_changeset` function should only require the `timeline_id`, `changeset`, and cloned `network` (or other minimal state) to perform its work.

## 2. Error Handling in Background Threads

### 2.1 Silent Panics in Server/Client Startup Threads
**Files:**
- `guihlp/src/ffi/internalserver.rs` (lines 56-76): `internal_server_start`
- `guihlp/src/ffi/state.rs` (lines 93-104): `client_state_bootstrap`
- `guihlp/src/ffi/state.rs` (lines 137-152): `client_state_network_run`

These functions spawn background threads that catch panics and log them, but return success to the caller immediately. If the thread panics (e.g., due to `tokio::runtime::Runtime::new` failing), the C++ caller receives a success indication while the actual initialization has failed.

**Recommendation:** 
- Propagate initialization errors synchronously where possible (e.g., by having the C++ side poll for readiness).
- If asynchronous initialization is required, use a callback or future mechanism to report success/failure to the C++ side.

## 3. Memory Management Burden on C++ Side

**Files:** Throughout `guihlp/include/esotereel_gui_helper.h`

The FFI interface transfers ownership of resources (strings, arrays, objects) to the C++ side via types like `FfiOwnedString`, `FfiArray`, and various `*_Result` types. While this is a common FFI pattern, it places the burden of correct resource management on the C++ side.

**Recommendation:**
- Ensure clear documentation in the C++ side about which functions require explicit cleanup (e.g., `owned_string_free`, `render_rows_free`, `wgpuutil_drop`).
- Consider providing RAII wrappers in the C++ code to automate resource management.

## 4. Plugin System Observations

**Files:** `lib/src/plugin/*`

No critical issues were found in the plugin system during a brief review. The plugin loading and field/application logic appears consistent with the architecture boundaries described in AGENTS.md (Rust core with FFI boundary).

## 5. General Code Health

### 5.1 Double Drain of Changes in `on_project_event`
**File:** `core/src/lib.rs` (lines 65-80)

The code drains changes from the project twice:
```rust
let mut changes = project.drain_changes();
// ...
changes.extend(project.drain_changes());
```
The comment indicates this is to capture changes from nested clips (Composite/Area/Script), but the implementation is unclear and may be inefficient or incorrect.

**Recommendation:** Clarify the intent and ensure the change drainage logic correctly captures all relevant changes without duplication or omission.

## Summary

The most critical issues are the FFI lifetime transmute (which, while likely safe, is a bad practice) and the potential deadlock in project locking (which could cause the application to hang). Addressing these will improve safety and adherence to the project's architecture guidelines.
