# Esotereel Architecture Documentation

## System Architecture

Esotereel implements a hybrid client-server architecture with clear separation between UI (C++/Qt) and business logic (Rust). This design provides type safety, memory safety, and performance while maintaining a rich native user interface.

## Architectural Layers

### 1. Presentation Layer (Qt6/C++)
**Location**: `gui/`

The presentation layer handles all user interaction and rendering:
- **Main Window**: Dock-based interface using Qt-Advanced-Docking-System
- **Timeline Widget**: Multi-track timeline editing interface
- **Preview Window**: Real-time video preview with wgpu rendering
- **Network Client**: TCP client for communicating with core server

**Key Components**:
- `MainWindow` - Main application window with dock management
- `TimelineWidget` - Timeline editing interface
- `RenderWorker` - Background rendering worker
- `WgpuCanvas` - WebGPU rendering surface
- `ClientNetworkHandler` - Network communication

### 2. Bridge Layer (Rust FFI)
**Location**: `guihlp/`

The bridge layer provides C-compatible interfaces between Qt and Rust:
- **FFI Functions**: C-exported functions for Qt to call
- **Type Wrappers**: Safe Rust wrappers around C types
- **Network Client**: Rust-based TCP client implementation
- **State Management**: Client-side state coordination

**Key Components**:
- `lib.rs` - FFI exports and error handling
- `network.rs` - Client network implementation
- `wrapper/` - C++ wrapper implementations
- `project.rs` - Project data structure wrappers

### 3. Business Logic Layer (Rust)
**Location**: `core/`

The core layer contains the main business logic and server implementation:
- **Server Application**: Main executable with tokio runtime
- **Network Server**: TCP server handling multiple clients
- **Project Management**: Project state and command handling
- **History System**: Undo/redo functionality

**Key Components**:
- `main.rs` - Server entry point
- `network.rs` - Server network implementation
- `project/` - Project management and commands

### 4. Shared Library Layer (Rust)
**Location**: `lib/`

The shared library contains common functionality used by both core and bridge:
- **Project Models**: Data structures for projects, timelines, clips
- **Rendering Engine**: wgpu-based GPU rendering
- **Video Decoding**: FFmpeg integration for video processing
- **Utilities**: Common utilities and type definitions

**Key Components**:
- `project/` - Project data structures and runtime
- `render/` - GPU rendering pipeline
- `decode/` - Video decoding and streaming
- `util/` - Utilities and helpers

## Communication Architecture

### Network Protocol

The system uses a custom binary protocol over TCP:

**Message Format**:
```
[4 bytes: length (little-endian)][N bytes: rkyv-serialized data]
```

**Request Types** (`lib/src/requests.rs`):
- `Test` - Connection testing
- `NewProject` - Create new project
- `ProjectAll` - Request full project state
- `Command` - Execute project command
- `InitStream` - Initialize video stream
- `FetchStreamData` - Fetch video data for a time range

**Response Types** (`lib/src/responces.rs`):
- `Test` - Test response
- `ProjectAll` - Full project state
- `ClipUpdates` - Incremental clip updates
- `StreamMetadata` - Video stream metadata
- `StreamData` - Video packet data
- `StreamDataEnd` - End of stream data notification

### Data Flow

**Request Flow (GUI → Core)**:
1. User action in Qt GUI
2. C++ wrapper calls Rust FFI function
3. Rust client serializes request using rkyv
4. TCP client sends to server
5. Server deserializes and processes request
6. Command pattern executes business logic
7. Response serialized and sent back

**Response Flow (Core → GUI)**:
1. Core processes request
2. Response serialized using rkyv
3. TCP server sends to client
4. Client deserializes response
5. FFI callback invoked
6. Qt GUI updated with new state

## Data Architecture

### Project Model

**Runtime Model** (`lib/src/project/`):
- `Project` - Runtime project state with timeline management
- `Timeline` - Runtime timeline with layer hierarchy and clip storage
- `Layer` - Layer with folder support and clip position tracking
- `Clip` - Individual media clip with transforms
- `ChangeSet` - Change tracking for network synchronization
- `ChunkIndex` - Spatial index for efficient time-range queries

**Key Features**:
- Timeline hierarchy with folder support (parent/children)
- Clip overlap detection and positioning
- Change tracking with upserted/removed clip tracking
- Efficient spatial queries using chunk-based indexing
- Network synchronization with metadata-based ProjectAll sync
- Independent timeline creation for composite clips

### Rendering Architecture

**GPU Rendering Pipeline** (`lib/src/render/`):
1. **Vertex Generation**: Build vertices from timeline state
2. **Batch Processing**: Group vertices by texture
3. **Buffer Updates**: Update GPU buffers with new data
4. **Render Pass**: Execute wgpu render commands
5. **Texture Copy**: Copy rendered frame to buffer

**Video Texture Management**:
- Stream-based texture updates
- Frame caching for performance
- Automatic texture cleanup

### Video Processing Architecture

**FFmpeg Integration** (`lib/src/decode/`):
- `VideoStreamer` - Server-side video decoding
- `StreamPlayer` - Client-side video playback
- Packet-based streaming for network efficiency
- Seek optimization with keyframe awareness

## Component Interactions

### Startup Sequence

1. **Core Server**:
   - `core/src/main.rs` starts tokio runtime
   - `server_network_start()` initializes TCP server
   - Listens on configured port (default: 12345)

2. **GUI Application**:
   - `gui/src/main.cpp` creates QApplication
   - Initializes Rust FFI callbacks
   - Starts internal server
   - Connects to core server
   - Shows main window

### Project Operations

**Creating a Project**:
1. GUI sends `NewProject` request
2. Core creates empty `Project` with default timeline
3. Core sends `ProjectAll` response with timeline metadata
4. GUI creates timeline structure from metadata (no clips yet)
5. GUI fetches clips as needed for visible range

**Adding a Clip**:
1. GUI creates clip data
2. Sends `Command::AddClip` request
3. Core executes command on project
4. Timeline tracks change in ChangeSet
5. Core sends `ClipUpdates` response
6. GUI updates timeline with new clip
7. Nested timeline changes are propagated to parent timelines

**Video Preview**:
1. GUI requests frame at specific time
2. Core calculates visible clips
3. VideoStreamer fetches required frames
4. Render pipeline generates frame
5. GPU renders to offscreen buffer
6. Frame data sent to GUI
7. GUI displays in preview window

## Technology Rationale

### Rust for Core Logic
- **Memory Safety**: Prevents common memory errors
- **Performance**: Zero-cost abstractions and efficient concurrency
- **Type System**: Strong typing prevents many bugs at compile time
- **Ecosystem**: Excellent crates for video processing and graphics

### Qt6 for GUI
- **Native Look**: Platform-native appearance and behavior
- **Rich Widgets**: Comprehensive set of UI components
- **Cross-Platform**: Write once, run anywhere
- **Mature**: Stable and well-documented framework

### rkyv for Serialization
- **Zero-Copy**: Deserialization without copying data
- **Performance**: Extremely fast serialization/deserialization
- **Type Safety**: Compile-time type checking
- **Compact**: Efficient binary representation

### wgpu for Rendering
- **Modern**: WebGPU standard for cross-platform graphics
- **Performance**: Hardware-accelerated rendering
- **Future-Proof**: Vendor-neutral API
- **Safe**: Rust wrapper around GPU APIs

### FFmpeg for Video
- **Comprehensive**: Supports most video formats and codecs
- **Performance**: Highly optimized video processing
- **Industry Standard**: Widely used in video applications
- **Flexible**: Extensive configuration options

## Concurrency Model

### Server Side (Core)
- **Tokio Runtime**: Async I/O for network operations
- **Mutex**: Protect shared state (ServerState)
- **Thread-per-Client**: Each client gets its own task
- **Channel-based**: mpsc channels for client communication

### Client Side (GUI)
- **Qt Event Loop**: Main thread for UI operations
- **Tokio Runtime**: Separate runtime for network I/O
- **Thread-safe**: Arc<Mutex<>> for shared state
- **Callback-based**: FFI callbacks for Rust → Qt communication

## Error Handling

### Rust Error Handling
- **Result Types**: Proper error propagation
- **Custom Errors**: Domain-specific error types
- **Logging**: Comprehensive logging via log crate
- **FFI Safety**: Error codes for C interop

### C++ Error Handling
- **Exceptions**: Standard C++ exception handling
- **Error Codes**: FFI error code propagation
- **Logging**: Qt logging integration
- **User Feedback**: Error messages in UI

## Performance Considerations

### Rendering Optimization
- **Batch Rendering**: Group draw calls by texture
- **Vertex Caching**: Reuse vertex data when possible
- **Texture Streaming**: Load video textures on demand
- **Offscreen Rendering**: Separate render thread

### Network Optimization
- **Binary Protocol**: Efficient binary serialization
- **Incremental Updates**: Only send changed data
- **Streaming**: Video data streamed in chunks
- **Zero-Copy**: rkyv avoids unnecessary copies

### Memory Management
- **Arc/Mutex**: Thread-safe shared ownership
- **Slot Maps**: Efficient entity management
- **Resource Cleanup**: Automatic resource management
- **Memory Pooling**: Reuse allocations when possible

## Security Considerations

### Network Security
- **Local Development**: Currently designed for local use
- **Input Validation**: Validate all network inputs
- **Resource Limits**: Prevent resource exhaustion attacks
- **Error Handling**: Safe error handling prevents information leakage

### Memory Safety
- **Rust Safety**: Rust prevents memory errors
- **FFI Boundaries**: Careful handling at FFI boundaries
- **Resource Management**: Proper cleanup of resources
- **Type Safety**: Strong typing prevents many vulnerabilities

## Extension Points

### Adding New Commands
1. Define command in `lib/src/project/commands.rs`
2. Implement command handler in `core/src/project/commands.rs`
3. Add FFI wrapper in `guihlp/src/wrapper/commands.rs`
4. Create C++ wrapper in `gui/src/wrapper/`
5. Ensure change tracking in Timeline (touch_upsert/touch_removed)

### Adding New Clip Types
1. Extend `ClipData` enum in `lib/src/project/clip.rs`
2. Implement rendering logic in `lib/src/render/`
3. Add UI components in `gui/src/`
4. Update serialization if needed

### Adding New Effects
1. Create effect shader in wgpu format
2. Integrate into render pipeline
3. Add effect parameters to clip transforms
4. Create UI controls for effect parameters

## Testing Strategy

### Unit Testing
- Rust unit tests for core logic
- Test command execution
- Test serialization/deserialization
- Test video processing

### Integration Testing
- Network communication tests
- FFI boundary tests
- End-to-end workflow tests
- Cross-language integration tests

### Performance Testing
- Rendering performance benchmarks
- Network latency measurements
- Memory usage profiling
- Video decoding performance

## Future Architectural Improvements

### Potential Enhancements
- **Plugin System**: Allow third-party extensions
- **GPU Compute**: Use compute shaders for effects
- **Distributed Processing**: Distribute rendering across machines
- **WebAssembly**: Enable web-based editing
- **Cloud Integration**: Cloud storage and collaboration
- **AI Features**: Automated editing assistance

### Technical Debt
- **Error Handling**: Improve error consistency across layers
- **Documentation**: Add more comprehensive API docs
- **Testing**: Increase test coverage
- **Performance**: Profile and optimize hot paths
- **Code Organization**: Further modularization where needed
- **Change Propagation**: Optimize nested timeline change propagation
