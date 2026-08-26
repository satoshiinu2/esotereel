# Esotereel Codebase Structure

## Directory Organization

The codebase is organized into four main components, each with distinct responsibilities:

```
esotereel/
├── gui/              # Qt6 C++ GUI Application
├── guihlp/           # Rust FFI Bridge Library
├── core/             # Rust Core Server Application
└── lib/              # Shared Rust Library
```

## GUI Component (`gui/`)

### Purpose
Qt6-based user interface application that provides the main editing environment.

### Structure
```
gui/
├── src/
│   ├── main.cpp                 # Application entry point
│   ├── log.cpp/h                # Logging utilities
│   ├── util.cpp/h               # General utilities
│   ├── meta_types_registration.cpp # Qt meta type registration
│   ├── network/
│   │   └── boot.cpp/h           # Network bootstrapping
│   ├── window/
│   │   ├── main.cpp/h           # Main window implementation
│   │   ├── timeline/
│   │   │   ├── timeline.cpp/h   # Timeline widget
│   │   │   ├── timeline_context.cpp  # Timeline context
│   │   │   ├── timeline_drag.cpp     # Drag and drop
│   │   │   ├── timeline_draw.cpp     # Rendering
│   │   │   ├── timeline_input.cpp    # Input handling
│   │   │   └── timeline_select.cpp   # Selection
│   │   ├── preview/
│   │   │   ├── render_worker.cpp/h  # Background rendering
│   │   │   └── wgpu_canvas.cpp/h     # WebGPU canvas
│   │   └── debug_streams.cpp/h   # Debug streams window
│   └── wrapper/
│       ├── exception.cpp/h      # Exception handling
│       ├── internalserver.cpp/h # Internal server management
│       ├── network.cpp/h         # Network wrapper
│       ├── requests.cpp/h        # Request handling
│       ├── stringview.h         # String view utilities
│       └── project/
│           ├── camera.cpp/h     # Camera controls
│           ├── clip.cpp/h       # Clip wrappers
│           ├── clip_render_info.cpp/h # Clip rendering info
│           ├── layer.cpp/h      # Layer wrappers
│           ├── layer_clips.cpp/h    # Layer-clip management
│           ├── project.cpp/h    # Project wrappers
│           └── timeline.cpp/h  # Timeline wrappers
```

### Key Files

#### `main.cpp`
- Entry point for the Qt application
- Initializes GUI callbacks
- Sets up network communication
- Creates and shows main window

#### `window/main.h/cpp`
- Main application window using Qt-Advanced-Docking-System
- Manages dock widgets for timeline, preview, etc.
- Coordinates between different UI components
- Handles window-level state management

#### `window/timeline/timeline.h/cpp`
- Multi-track timeline editing interface
- Handles clip manipulation (drag, drop, resize)
- Manages timeline rendering and input
- Provides selection and editing capabilities

#### `window/preview/wgpu_canvas.h/cpp`
- WebGPU rendering surface for video preview
- Handles GPU context management
- Provides frame display functionality
- Integrates with render worker

#### `wrapper/network.h/cpp`
- C++ wrapper for Rust network client
- Provides type-safe interface to Rust FFI
- Handles request/response serialization
- Manages network state and callbacks

## GUI Helper Component (`guihlp/`)

### Purpose
Rust library that provides C-compatible FFI interface between Qt GUI and Rust core.

### Structure
```
guihlp/
├── include/              # C header files for FFI
├── src/
│   ├── lib.rs            # FFI exports and core functionality
│   ├── network.rs        # Client network implementation
│   ├── project.rs        # Project data structure wrappers
│   ├── responces.rs      # Response handling
│   └── wrapper/
│       ├── mod.rs        # Wrapper module exports
│       ├── commands.rs   # Command wrappers
│       ├── debug_streams.rs # Debug stream utilities
│       ├── internalserver.rs # Internal server management
│       ├── logger.rs     # Logging integration
│       ├── network.rs    # Network wrappers
│       ├── render.rs     # Render frame FFI wrapper
│       ├── stringview.rs # String view utilities
│       ├── wgpuutil.rs   # WebGPU utilities
│       └── project/
│           ├── mod.rs    # Project wrapper exports
│           ├── clip.rs   # Clip wrappers
│           ├── clip_render_info.rs # Clip rendering info
│           ├── debug.rs  # Debug utilities
│           ├── layer.rs  # Layer wrappers
│           └── timeline.rs # Timeline wrappers
```

### Key Files

#### `lib.rs`
- Main FFI entry point
- Exports C-compatible functions
- Error handling with `WrapperErrorCode`
- GUI callback management
- Thread-local error message storage

#### `network.rs`
- Tokio-based TCP client implementation
- Handles connection to core server
- Request/response serialization using rkyv
- Manages client state and callbacks

#### `project.rs`
- Project data structure wrappers
- Provides safe Rust interfaces for C++ code
- Manages project state synchronization

#### `wrapper/commands.rs`
- Command pattern implementation for FFI
- Type-safe command execution
- Error handling and validation

#### `wrapper/render.rs`
- FFI wrapper for render_frame_offscreen
- Safe interface to GPU rendering
- Error handling with panic recovery
- Output buffer management

## Core Component (`core/`)

### Purpose
Rust application containing core business logic and server implementation.

### Structure
```
core/
└── src/
    ├── main.rs                # Server entry point
    ├── lib.rs                 # Core library exports
    ├── network.rs             # Server network implementation
    ├── requests.rs            # Request handling
    └── project/
        ├── mod.rs             # Project module exports
        ├── commands.rs        # Command execution
        └── history.rs         # Undo/redo history
```

### Key Files

#### `main.rs`
- Tokio runtime entry point
- Logger initialization
- Server network startup
- Default server address: 0.0.0.0:12345

#### `network.rs`
- Tokio-based TCP server implementation
- Handles multiple client connections
- Request parsing and routing
- Response broadcasting
- Global instance management for FFI callbacks

#### `project/commands.rs`
- Command pattern implementation
- Project modification commands
- Validation and execution logic
- History integration

#### `project/history.rs`
- Undo/redo functionality
- Command history management
- State rollback capabilities

## Shared Library Component (`lib/`)

### Purpose
Shared Rust library containing common functionality used by both core and guihlp.

### Structure
```
lib/
└── src/
    ├── lib.rs                # Library entry point and state management
    ├── requests.rs           # Request type definitions
    ├── responces.rs          # Response type definitions
    ├── project/
    │   ├── mod.rs            # Project module exports
    │   ├── camera.rs         # Camera implementation
    │   ├── change.rs         # Change tracking for synchronization
    │   ├── chunk_index.rs    # Spatial index for clip queries
    │   ├── clip.rs           # Clip data structures
    │   ├── commands.rs       # Command definitions
    │   ├── ids.rs            # ID generation and management
    │   ├── layer.rs          # Layer data structures
    │   ├── project.rs        # Project runtime implementation
    │   ├── save.rs           # Project save/load
    │   ├── timeline.rs       # Timeline runtime implementation
    │   ├── transform.rs      # Transform data structures
    │   └── util.rs           # Project utilities
    ├── render/
    │   ├── mod.rs            # Render module exports
    │   ├── builder.rs        # Vertex building
    │   ├── pipeline.rs       # Render pipeline
    │   ├── surfacetarget.rs  # Surface target management
    │   ├── uniform.rs        # Uniform buffer management
    │   ├── vertex.rs         # Vertex data structures
    │   ├── wgpuutil.rs       # WebGPU utilities
    │   └── video/
    │       ├── mod.rs        # Video render module
    │       └── request.rs    # Video render requests
    ├── decode/
    │   ├── mod.rs            # Decode module exports
    │   ├── streamplayer.rs   # Client-side stream player
    │   └── videostreamer.rs  # Server-side video streamer
    └── util/
        ├── mod.rs            # Utility module exports
        ├── logger.rs         # Logging utilities
        ├── order_map.rs      # Ordered map implementation
        ├── result.rs         # Result types
        ├── slot_map.rs       # Slot map implementation
        └── types.rs          # Common types
```

### Key Files

#### `lib.rs`
- Library entry point
- State management (`ClientState`, `ServerState`)
- FFI callback management
- Stream state management
- Thread-safe state structures

#### `requests.rs`
- Request type definitions using rkyv
- Test, NewProject, ProjectAll, Command, InitStream, FetchStreamData
- Serialization support

#### `responces.rs`
- Response type definitions using rkyv
- Test, ProjectAll, ClipUpdates, StreamMetadata, StreamData, StreamDataEnd
- Serialization support

#### `project/mod.rs`
- Project module exports
- Common project types
- Clip update map types

#### `project/clip.rs`
- Clip data structure with rkyv/serde support
- ClipData enum (Dummy, Video, Audio, Composite, Area2D, Area3D)
- Transform data
- Time calculation utilities

#### `project/project.rs`
- Project runtime implementation
- Timeline management with BTreeMap
- ID generation and observation
- Independent timeline creation (deep clone)
- Change set collection and propagation
- Timeline metadata for ProjectAll sync

#### `project/timeline.rs`
- Timeline runtime implementation
- Layer hierarchy management (folder support)
- Clip storage and positioning
- Change tracking (ChangeSet)
- ChunkIndex for spatial queries
- Clip overlap detection
- Range queries for visible clips
- Network sync support (merge_fetched_clips, upsert_clip_from_network)

#### `project/layer.rs`
- Layer data structures
- Folder hierarchy support (children)
- Clip position tracking (BTreeMap)
- Layer metadata for network sync

#### `project/change.rs`
- Change tracking for synchronization
- ChangeSet with upserted/removed clip tracking
- Change merging for nested timeline propagation

#### `project/chunk_index.rs`
- Spatial index for efficient time-range queries
- Chunk-based clip lookup
- Lazy rebuilding when invalidated

#### `render/mod.rs`
- Main render function: `render_frame_offscreen`
- Render batch management
- Camera integration
- Vertex building coordination

#### `render/wgpuutil.rs`
- WebGPU utility implementation
- Device and queue management
- Buffer and texture management
- Render pass management

#### `decode/videostreamer.rs`
- FFmpeg-based video decoding
- Stream metadata extraction
- Packet reading and processing
- Seek functionality
- Frame decoding and scaling

#### `decode/streamplayer.rs`
- Client-side video playback
- Stream management
- Frame caching
- Playback control

## Data Flow Between Components

### Request Flow
```
Qt GUI → C++ Wrapper → Rust FFI → Client Network → TCP → Server Network → Core Logic
```

### Response Flow
```
Core Logic → Server Network → TCP → Client Network → Rust FFI → C++ Wrapper → Qt GUI
```

## Build Artifacts

### Rust Components
- `lib/target/debug/` or `lib/target/release/`
  - `libesotereel_lib.rlib` - Shared library
  - `libesotereel_gui_helper.so/dylib/dll` - FFI library
  - `esotereel_core` - Core server executable

### C++ Components
- `build/` directory
  - `esotereel_gui` - Main GUI executable

## Configuration Files

### `Cargo.toml` (Root)
- Rust workspace configuration
- Member crates: lib, guihlp, core
- Dependency resolver settings

### `CMakeLists.txt`
- CMake build configuration
- Qt6 integration
- Rust build integration
- Platform-specific settings
- clangd configuration generation

### `.clangd`
- Auto-generated clangd configuration
- Include paths and compiler flags
- Platform-specific settings

## Development Workflow

### Adding New Features
1. Define data structures in `lib/`
2. Implement business logic in `core/`
3. Create FFI wrappers in `guihlp/`
4. Build UI components in `gui/`
5. Test integration between layers

### Debugging
- Rust: Use `println!` and `log::` macros
- C++: Use Qt logging and qDebug
- Network: Monitor TCP traffic
- FFI: Check error codes and messages

### Testing
- Unit tests in Rust modules
- Integration tests for network communication
- Manual testing through GUI
- Performance profiling for rendering

## Dependencies Summary

### External Dependencies
- **Qt6**: GUI framework
- **FFmpeg**: Video processing
- **wgpu**: Graphics rendering
- **tokio**: Async runtime

### Internal Dependencies
- `core` depends on `lib`
- `guihlp` depends on `lib` and `core`
- `gui` depends on `guihlp` (via FFI)

This structure provides clear separation of concerns while maintaining efficient communication between components through well-defined interfaces and protocols.
