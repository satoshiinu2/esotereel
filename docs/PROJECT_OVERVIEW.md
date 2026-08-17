# Esotereel Project Overview

## Project Description

Esotereel is a video editing software application with a hybrid architecture combining Qt (C++) for the user interface and Rust for core functionality. The project uses a client-server architecture where the GUI (Qt) communicates with core processing logic (Rust) via TCP/IP networking.

## Project Status

Currently under active development and construction.

## Key Technologies

- **GUI Framework**: Qt6 (C++)
- **Core Logic**: Rust (2024 edition)
- **Graphics Rendering**: wgpu (WebGPU)
- **Video Processing**: FFmpeg (via ffmpeg-next)
- **Serialization**: rkyv (zero-copy deserialization)
- **Build System**: CMake with Rust integration
- **IDE Support**: clangd for C++, rust-analyzer for Rust

## Architecture Overview

The project follows a three-tier architecture:

1. **GUI Layer** (`gui/`): Qt6-based user interface
2. **Bridge Layer** (`guihlp/`): Rust FFI library providing C-compatible interface
3. **Core Layer** (`core/`): Rust application containing business logic
4. **Shared Library** (`lib/`): Common Rust code shared between core and bridge

## Project Structure

```
esotereel/
├── gui/                 # Qt6 C++ GUI application
│   └── src/
│       ├── main.cpp     # Application entry point
│       ├── window/      # UI components (timeline, preview, etc.)
│       ├── network/     # Network communication with core
│       └── wrapper/     # C++ wrappers for Rust FFI
├── guihlp/              # Rust FFI library (cdylib)
│   ├── include/         # C header files for FFI
│   └── src/             # Rust implementation
├── core/                # Rust core application
│   └── src/
│       ├── main.rs      # Core server entry point
│       ├── network.rs   # Network server implementation
│       └── project/     # Project management logic
├── lib/                 # Shared Rust library
│   └── src/
│       ├── project/     # Project data structures
│       ├── render/      # GPU rendering (wgpu)
│       ├── decode/      # Video decoding (FFmpeg)
│       └── util/        # Utilities
├── CMakeLists.txt       # Build configuration
└── Cargo.toml           # Rust workspace configuration
```

## Build System

The project uses CMake as the primary build system with integrated Rust compilation:

```bash
cmake -G Ninja -S '${workspaceFolder}' -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

CMake automatically:
- Builds Rust workspace (lib, guihlp, core)
- Compiles Qt6 C++ application
- Links against Rust libraries
- Generates .clangd configuration for IDE support

## Communication Protocol

The GUI and Core communicate via TCP/IP using a custom binary protocol:
- **Serialization**: rkyv for zero-copy deserialization
- **Transport**: TCP on port 12345 (default)
- **Message Types**: Defined in `lib/src/requests.rs` and `lib/src/responces.rs`

## Key Features

1. **Multi-timeline Editing**: Support for multiple timelines with layers
2. **Video Processing**: FFmpeg-based video decoding and streaming
3. **GPU Rendering**: wgpu-based hardware accelerated rendering
4. **Clip Management**: sophisticated clip system with transforms
5. **Real-time Preview**: Offscreen rendering for timeline preview
6. **Network Architecture**: Client-server model for scalability

## Development Environment

- **C++ Standard**: C++20
- **Rust Edition**: 2024
- **Qt Version**: Qt6 (with Wayland support on Linux)
- **Platform Support**: Linux, Windows, macOS
- **Debug Tools**: Address sanitizer in debug builds

## Dependencies

### Rust Dependencies
- `wgpu` - Graphics rendering
- `ffmpeg-next` - Video processing
- `rkyv` - Serialization
- `tokio` - Async runtime
- `serde` - JSON serialization
- `glam` - Math library
- `dashmap` - Concurrent data structures

### C++ Dependencies
- Qt6 (Widgets, Network, GUI, Core, LinguistTools)
- Qt-Advanced-Docking-System - Docking UI components

## Language Support

The project includes internationalization support with translation files:
- `esotereel_ja.ts` - Japanese translation
- `esotereel_en.ts` - English translation

## Future Development

As this is an active project under construction, future areas of development may include:
- Enhanced video effects and filters
- Audio processing capabilities
- Export functionality
- Plugin system
- Performance optimizations
