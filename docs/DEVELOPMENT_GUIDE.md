# Esotereel Development Guide

This guide provides practical information for developers, LLMs, and Agents working with the Esotereel codebase.

## Build Instructions

### Prerequisites
- Rust toolchain (2024 edition)
- CMake (3.16+)
- Qt6 development packages
- C++ compiler with C++20 support
- Ninja build system (recommended)
- FFmpeg development libraries

### Building the Project

```bash
# Configure build
cmake -G Ninja -S '${workspaceFolder}' -B build -DCMAKE_BUILD_TYPE=Release

# Build all components
cmake --build build

# Debug build with address sanitizer
cmake -G Ninja -S '${workspaceFolder}' -B build -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

### Build Components
The build process compiles:
1. Rust workspace (lib, guihlp, core)
2. Qt6 C++ application (gui)
3. Links components together
4. Generates IDE configuration (.clangd)

## Development Workflow

### Project Structure Understanding
1. **Shared Library (lib/)**: Start here for data structures and core logic
2. **Core Application (core/)**: Server-side business logic
3. **GUI Helper (guihlp/)**: FFI bridge between Qt and Rust
4. **GUI Application (gui/)**: User interface in Qt/C++

### Typical Feature Development

**Adding a New UI Feature:**
1. Design data structures in `lib/src/project/`
2. Implement business logic in `core/src/project/`
3. Create FFI wrappers in `guihlp/src/wrapper/`
4. Build Qt UI components in `gui/src/`
5. Test integration between layers

**Adding Video Processing:**
1. Extend FFmpeg integration in `lib/src/decode/`
2. Update streaming logic in core
3. Add client-side handling in guihlp
4. Integrate with rendering pipeline

**Adding Rendering Effects:**
1. Create shader logic in `lib/src/render/`
2. Update vertex/pipeline code
3. Add parameters to clip transforms
4. Create UI controls in Qt

## Code Conventions

### Rust Code Style
- Use 2024 edition features
- Follow standard Rust naming conventions
- Prefer `Arc<Mutex<T>>` for shared state
- Use `DashMap` for concurrent hash maps
- Implement proper error handling with `Result<T, E>`
- Add `#[derive(Debug)]` for debugging
- Use rkyv for network serialization

### C++ Code Style
- Use C++20 features
- Follow Qt naming conventions (camelCase for methods)
- Use RAII for resource management
- Prefer Qt containers over STL
- Use smart pointers (std::shared_ptr, std::unique_ptr)
- Implement proper exception handling

### FFI Safety
- All FFI functions must be `extern "C"`
- Use `unsafe` blocks appropriately
- Validate pointers before dereferencing
- Handle C strings carefully
- Provide error codes for C callers
- Use thread-local storage for error messages

## Testing Strategy

### Unit Testing (Rust)
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_clip_creation() {
        // Test implementation
    }
}
```

### Integration Testing
- Test network communication between GUI and core
- Test FFI boundary correctness
- Test serialization/deserialization
- Test command execution

### Manual Testing
- Test UI workflows through the application
- Test video processing with various formats
- Test rendering with different projects
- Test network resilience

## Debugging

### Rust Debugging
```rust
// Use logging
log::info!("Debug message: {}", value);
log::error!("Error occurred: {:?}", error);

// Use println for quick debugging
println!("Debug: {:?}", data);
```

### C++ Debugging
```cpp
// Use Qt logging
qDebug() << "Debug message:" << value;
qWarning() << "Warning:" << message;
qCritical() << "Error:" << error;

// Use standard output
std::cout << "Debug: " << value << std::endl;
```

### Network Debugging
- Monitor TCP traffic with Wireshark
- Check request/response serialization
- Verify callback invocation
- Test connection handling

### FFI Debugging
- Check error codes with `get_last_err_msg()`
- Verify callback registration
- Test data marshaling
- Monitor thread safety

## Performance Optimization

### Rendering Optimization
- Batch draw calls by texture
- Reuse vertex data when possible
- Implement texture streaming
- Use offscreen rendering
- Profile GPU usage

### Network Optimization
- Use rkyv for zero-copy deserialization
- Send incremental updates only
- Stream video data in chunks
- Minimize round trips
- Implement compression if needed

### Memory Optimization
- Use slot maps for entity management
- Implement resource pooling
- Clean up unused resources
- Profile memory usage
- Use efficient data structures

## Common Patterns

### State Management
```rust
// Shared state pattern
pub struct ServerState {
    pub project: Arc<RwLock<Option<Project>>>,
    pub streams: Arc<DashMap<u32, VideoStreamer>>,
}
```

### Command Pattern
```rust
// Command execution
pub enum Command {
    AddClip { /* parameters */ },
    RemoveClip { /* parameters */ },
    // ... other commands
}
```

### FFI Pattern
```rust
// FFI function pattern
#[unsafe(no_mangle)]
pub unsafe extern "C" fn function_name(params) -> WrapperErrorCode {
    // Implementation
}
```

### Error Handling
```rust
// Result pattern
pub type EsotereelResult<T> = Result<T, EsotereelError>;

pub fn operation() -> EsotereelResult<()> {
    // Implementation
    Ok(())
}
```

## IDE Configuration

### VS Code
- Install rust-analyzer for Rust
- Install C/C++ extension for C++
- Install Qt extension for Qt development
- Use .clangd configuration for C++ IntelliSense

### CLion
- Configure Rust toolchain
- Configure CMake build
- Set up Qt installation
- Configure .clangd for code completion

### Vim/Neovim
- Use rust-analyzer via LSP
- Use clangd for C++ completion
- Configure build commands
- Set up debugging integration

## Common Issues and Solutions

### Build Issues
- **Rust compilation errors**: Check Cargo.toml dependencies
- **CMake configuration errors**: Verify Qt6 installation
- **Linking errors**: Check library paths and order
- **FFmpeg errors**: Verify FFmpeg development packages

### Runtime Issues
- **Connection failures**: Check server is running and port is available
- **Rendering issues**: Verify wgpu backend and drivers
- **Video decoding errors**: Check FFmpeg codec support
- **Memory leaks**: Use appropriate smart pointers

### FFI Issues
- **Callback not invoked**: Check callback registration
- **Data corruption**: Verify serialization/deserialization
- **Thread safety**: Use proper synchronization primitives
- **String handling**: Check C string conversion

## Best Practices

### Code Organization
- Keep modules focused and small
- Use clear naming conventions
- Document public APIs
- Separate concerns between layers
- Minimize dependencies

### Error Handling
- Handle errors gracefully
- Provide meaningful error messages
- Use appropriate error types
- Log errors for debugging
- Clean up resources on errors

### Performance
- Profile before optimizing
- Focus on hot paths
- Use appropriate data structures
- Minimize allocations
- Leverage Rust's zero-cost abstractions

### Security
- Validate all inputs
- Handle FFI boundaries carefully
- Use safe Rust where possible
- Minimize unsafe code
- Review dependencies

## Git Workflow

### Commit Messages
Follow conventional commit format:
```
feat: add new timeline feature
fix: resolve video decoding issue
docs: update API documentation
refactor: improve render pipeline
```

### Branch Strategy
- `main`: Stable development branch
- `feature/*`: Feature development
- `bugfix/*`: Bug fixes
- `docs/*`: Documentation updates

## Documentation

### Code Documentation
- Document public APIs with Rust doc comments
- Add comments for complex logic
- Document FFI functions thoroughly
- Include usage examples

### API Documentation
- Keep API reference up to date
- Document new functions and types
- Update architectural diagrams
- Maintain development guides

## Collaboration

### Code Review
- Review changes across language boundaries
- Check FFI safety
- Verify serialization compatibility
- Test integration points
- Review performance implications

### Issue Tracking
- Use descriptive issue titles
- Include reproduction steps
- Provide environment details
- Attach logs and error messages
- Label issues appropriately

## Continuous Integration

### Build Verification
- Test on multiple platforms
- Verify all build configurations
- Run automated tests
- Check for memory leaks
- Validate FFI boundaries

### Quality Checks
- Run clippy for Rust
- Use static analysis for C++
- Check code formatting
- Verify documentation builds
- Test with sanitizers

## Learning Resources

### Rust Resources
- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### Qt Resources
- [Qt Documentation](https://doc.qt.io/)
- [Qt Best Practices](https://wiki.qt.io/Best_Practices)
- [Qt Examples](https://doc.qt.io/qt-6/examples.html)

### FFmpeg Resources
- [FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- [FFmpeg Examples](https://ffmpeg.org/doxygen/trunk/examples.html)

### wgpu Resources
- [WebGPU Specification](https://www.w3.org/TR/webgpu/)
- [wgpu-rs Documentation](https://docs.rs/wgpu/)
- [WebGPU Samples](https://webgpu.github.io/webgpu-samples/)

## Troubleshooting Guide

### Environment Setup Issues
- Qt6 not found: Set Qt6_DIR environment variable
- Rust toolchain issues: Use rustup to manage toolchain
- CMake configuration fails: Check CMake version and dependencies
- FFmpeg linking errors: Verify FFmpeg development packages

### Development Issues
- Hot reload not working: Restart server and client
- Changes not reflected: Clean build directory
- Debugger not attaching: Check debug build configuration
- IntelliSense not working: Regenerate .clangd configuration

### Performance Issues
- Slow rendering: Profile GPU usage and optimize shaders
- High memory usage: Check for memory leaks and optimize allocations
- Network latency: Optimize serialization and reduce message size
- Video decoding slow: Check FFmpeg configuration and codec support

## Future Development

### Planned Features
- Enhanced audio processing
- More video effects and filters
- Export functionality
- Plugin system
- Cloud integration
- Collaboration features

### Technical Improvements
- Better error handling
- Increased test coverage
- Performance optimizations
- Code modularization
- Documentation improvements

This development guide provides practical guidance for working with the Esotereel codebase. For specific technical details, refer to the API reference and architecture documentation.
