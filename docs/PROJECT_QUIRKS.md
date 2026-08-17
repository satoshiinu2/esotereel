# Esotereel Project Quirks and Implementation Details

This document describes specific quirks, implementation details, and unique characteristics of the Esotereel codebase that may not be immediately obvious from reading the code.

## Architecture Quirks

### Dual Runtime Model
The project runs **two separate tokio runtimes**:
- **Core Server**: Runs in `core/src/main.rs` with its own tokio runtime
- **GUI Helper**: Runs in `guihlp/` with its own tokio runtime for network client

This is unusual because typically you'd have a single runtime, but the client-server architecture necessitates separate runtimes.

### Internal Server Pattern
The GUI starts an **internal server** that connects to the core server:
- `gui/src/main.cpp` starts `InternalServer::start()`
- This internal server then connects to the external core server
- Creates a proxy-like pattern for local development

### FFI Callback Chain
Rust → Qt communication uses a **complex callback chain**:
1. Rust code calls C function pointer
2. C function calls Qt slot/member function
3. Qt function updates UI
4. This requires careful thread management and GUI thread synchronization

## Data Structure Quirks

### Dual Model System
The project maintains **two parallel project models**:
- **Domain Model** (`lib/src/project/model/`): For serialization and storage
- **Runtime Model** (`lib/src/project/runtime/`): For active editing

This separation is intentional:
- Domain models use rkyv for serialization
- Runtime models use optimized structures for editing
- Conversion functions between the two models

### BTreeMap Preference
The codebase heavily prefers **BTreeMap over HashMap**:
- `ProjectModel.timelines: BTreeMap<u64, TimelineModel>`
- `TimelineModel.layers: BTreeMap<u32, LayerModel>`

Reason: Ordered iteration is frequently needed for rendering and UI display.

### ID Generation Pattern
The project uses a **centralized ID generator** rather than letting each component generate its own IDs:
- Single `IdGenerator` per project
- Ensures ID uniqueness across different entity types
- Requires careful state management during deserialization

## Network Quirks

### Custom Binary Protocol
Instead of using established protocols like HTTP/WebSocket, the project uses a **custom binary protocol**:
- Length-prefixed messages (4 bytes little-endian)
- rkyv serialization for payload
- TCP as transport

This provides maximum performance but requires custom tooling for debugging.

### Global Instance Pattern
Network handlers use **global static instances** for FFI callback access:
```rust
pub static INSTANCE: RwLock<Option<Arc<ServerNetworkHandler>>> = RwLock::new(None);
```

This is an anti-pattern in modern Rust but necessary for C callback integration.

### Unbounded Channels
The project uses **unbounded mpsc channels** for network communication:
```rust
type ClientSender = mpsc::UnboundedSender<AlignedVec>;
```

This can lead to unbounded memory growth if backpressure isn't managed elsewhere.

## FFI Quirks

### Manual Send/Sync Implementations
Several types have **manual Send/Sync implementations**:
```rust
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
```

This is potentially unsafe and requires careful audit of the underlying types.

### Thread-Local Error Storage
Error messages are stored in **thread-local storage**:
```rust
thread_local! {
    static LAST_ERR_MSG: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}
```

This means error messages are only valid on the same thread that set them.

### C String Handling
The project uses **CString extensively** for FFI string handling:
- Requires careful memory management
- FFI boundaries must handle string conversion
- Potential for encoding issues

## Rendering Quirks

### Offscreen Rendering Pattern
Rendering uses an **offscreen texture pattern**:
- Render to texture first
- Copy texture to buffer
- Send buffer data to GUI
- GUI displays the buffer

This is more complex than direct rendering but provides better network integration.

### Vertex Builder Pattern
The project uses a **custom vertex builder** rather than a standard 3D engine:
- Manual vertex construction from timeline state
- Custom batching by texture
- Direct GPU buffer management

This gives maximum control but requires deep graphics knowledge.

### Video Texture Streaming
Video textures are **streamed frame-by-frame**:
- Not cached as full video in GPU memory
- Each frame decoded and uploaded as needed
- Requires careful memory management

## FFmpeg Integration Quirks

### Unsafe FFmpeg Calls
The FFmpeg integration uses **unsafe calls to FFmpeg internals**:
```rust
unsafe {
    let codec_context = self.decoder.as_ptr();
    let data = (*codec_context).extradata;
    // Direct pointer access
}
```

This bypasses FFmpeg's safe wrappers for performance but is inherently unsafe.

### Custom Seek Logic
The project implements **custom seek logic** rather than using FFmpeg's built-in seek:
- Manual timestamp calculation
- Keyframe awareness
- Discontinuity flag handling

This provides better control but is complex to maintain.

## Build System Quirks

### CMane Integration with Cargo
The project uses **CMake to drive Cargo builds**:
- CMake calls `cargo build` as a custom command
- CMake generates .clangd configuration
- Mixed build system is complex to debug

### Dynamic Library Loading
The GUI dynamically loads the Rust library:
- `guihlp` compiled as cdylib
- Loaded at runtime by Qt application
- Requires careful symbol management

## Error Handling Quirks

### Multiple Error Systems
The project uses **multiple error handling systems**:
- Rust: `Result<T, E>` types
- C++: Exceptions
- FFI: `WrapperErrorCode` enum
- Each layer has its own error handling approach

### Silent Failures
Some operations have **silent failure modes**:
- Network send failures log but don't propagate
- FFI errors set thread-local state but may not be checked
- Rendering failures may result in blank frames without user notification

## Performance Quirks

### Zero-Copy Emphasis
The project heavily emphasizes **zero-copy operations**:
- rkyv for zero-copy deserialization
- Slice references instead of owned data
- Direct buffer access

This provides performance but increases complexity.

### Cache-Heavy Design
The rendering system is **cache-heavy**:
- Texture caching
- Vertex buffer reuse
- Frame caching for video

This improves performance but increases memory usage.

## State Management Quirks

### RwLock Over Mutex
The project prefers **RwLock over Mutex** for shared state:
- Allows multiple concurrent readers
- Single writer for modifications
- Fits the read-heavy workload pattern

### Arc Everywhere
Shared state uses **Arc extensively**:
- Arc<Mutex<T>> for exclusive access
- Arc<RwLock<T>> for shared access
- Arc<DashMap> for concurrent maps

This provides thread safety but adds overhead.

## Testing Quirks

### Limited Test Coverage
The project has **limited automated test coverage**:
- Few unit tests in Rust code
- No integration tests for network communication
- Manual testing through GUI

This is a known area for improvement.

### Debug Build Sanitizers
Debug builds enable **address sanitizer**:
```cmake
if (CMAKE_BUILD_TYPE STREQUAL "Debug")
    target_compile_options(esotereel_gui PRIVATE -fsanitize=address -fno-omit-frame-pointer)
    target_link_options(esotereel_gui PRIVATE -fsanitize=address)
endif()
```

This helps catch memory bugs but affects performance.

## Qt Integration Quirks

### Docking System
The project uses **Qt-Advanced-Docking-System**:
- Custom docking library
- All warnings disabled for this library
- Version pinned to 5.0.0

This provides advanced docking but adds external dependency.

### Wayland Support
Wayland support is **conditionally included**:
```cmake
if(UNIX AND NOT APPLE)
    list(APPEND QT_COMPONENTS WaylandClient)
endif()
```

This provides modern Linux support but adds platform complexity.

## Code Organization Quirks

### Mixed Language Modules
Some functionality is **split across languages**:
- Project data structures in Rust
- Project wrappers in C++
- FFI bridge in Rust
- UI in C++

This requires understanding multiple languages for single features.

### Forward Declaration Headers
The project uses **forward declaration headers** (.fwd.h):
- Reduce compilation dependencies
- Separate interface from implementation
- Additional files to maintain

## Concurrency Quirks

### Thread-Per-Client Model
The server uses a **thread-per-client model**:
- Each client connection spawns new tasks
- No connection pooling
- Simple but potentially resource-intensive

### GUI Thread Assumptions
The code assumes **GUI operations on main thread**:
- Qt GUI must run on main thread
- Rust callbacks must be thread-safe
- Manual thread synchronization required

## Memory Management Quirks

### Manual Resource Cleanup
Some resources require **manual cleanup**:
- FFmpeg contexts
- GPU resources
- Network connections

This increases complexity but provides fine control.

### Leak Potential
The complex FFI boundaries and callback chains create **potential for memory leaks**:
- Callback ownership unclear
- Resource cleanup timing
- Exception safety at boundaries

## Debugging Quirks

### Complex Call Chains
Debugging requires understanding **complex call chains**:
- Qt event loop → C++ wrapper → Rust FFI → Network → Core → Network → Rust FFI → C++ → Qt
- Each layer may transform data
- Error propagation across language boundaries

### Limited Debugging Tools
Standard debugging tools have **limited effectiveness**:
- GDB/lldb struggle with FFI boundaries
- Rust debugger doesn't understand Qt
- Qt debugger doesn't understand Rust

## Evolution Artifacts

### Legacy Code Patterns
Some code shows **evolution and refactoring**:
- Runtime vs domain model split suggests iteration
- Multiple error systems suggest architectural changes
- Global instances suggest original design constraints

### Inconsistent Patterns
Not all code follows consistent patterns:
- Some areas use modern Rust idioms
- Others use more traditional approaches
- C++ code varies in style

## Dependencies Quirks

### Pinned Dependency Versions
Some dependencies are **pinned to specific versions**:
- Qt-Advanced-Docking-System: 5.0.0
- Rust edition: 2024 (bleeding edge)

This can cause compatibility issues but ensures stability.

### Feature-Rich Dependencies
Some dependencies use **many features**:
- tokio with "full" features
- wgpu with all capabilities
- This increases build time and binary size

## Platform-Specific Quirks

### Windows Handling
Windows has **special handling**:
- DLL deployment scripts
- Different library extensions
- Unicode defines for clangd

### Linux Wayland
Linux has **Wayland-specific code**:
- Additional Qt components
- Platform-specific includes
- Conditional compilation

## Development Workflow Quirks

### Two-Step Build Process
Building requires **two separate steps**:
1. CMake configuration
2. CMake build (which triggers Cargo builds)

This is more complex than single-language projects.

### IDE Configuration
IDE support requires **special configuration**:
- .clangd auto-generated by CMake
- rust-analyzer needs workspace configuration
- Qt requires specific plugins

## Security Quirks

### Local Network Assumption
The project assumes **local network usage**:
- No authentication
- No encryption
- No input validation beyond basic checks

This is acceptable for local development but not for production deployment.

### Unsafe Code Blocks
The project contains **significant unsafe code**:
- FFI boundaries
- FFmpeg integration
- GPU operations

This requires careful security review.

## Performance Characteristics

### GPU Bottleneck
Rendering is often **GPU-bound**:
- Complex shaders can bottleneck
- Texture upload can be slow
- Resolution significantly impacts performance

### Network Latency
The client-server architecture introduces **network latency**:
- Every UI action requires network round-trip
- Local network minimizes but doesn't eliminate this
- Video streaming can be bandwidth-intensive

## Future Considerations

### Scalability Limits
Current architecture has **scalability limitations**:
- Single server instance
- No connection pooling
- Thread-per-client model

### Maintainability Concerns
The complex architecture creates **maintainability challenges**:
- Multiple languages to understand
- Complex call chains
- Limited test coverage

Understanding these quirks is essential for effectively working with the Esotereel codebase, especially when making architectural decisions or debugging complex issues.
