# Esotereel Documentation

This directory contains comprehensive documentation for the Esotereel video editing software project, designed for LLMs, Agents, and developers working with the codebase.

## Documentation Structure

### 📋 Product Documentation
**Start here for understanding the product**
- [product/overview.md](product/overview.md) - Product overview and implemented features
- [product/concepts.md](product/concepts.md) - Domain concepts and their relationships
- [product/feature-map.md](product/feature-map.md) - Feature map and dependencies

### 🎯 Use Case Documentation
**Detailed user interaction flows**
- [usecases/new-project.md](usecases/new-project.md) - New project creation
- [usecases/add-clip.md](usecases/add-clip.md) - Adding clips to timeline
- [usecases/clips-move.md](usecases/clips-move.md) - Moving clips
- [usecases/add-layer-folder.md](usecases/add-layer-folder.md) - Adding layers and folders
- [usecases/init-stream.md](usecases/init-stream.md) - Video stream initialization
- [usecases/fetch-clips-in-range.md](usecases/fetch-clips-in-range.md) - Fetching clips in range

### 🏗️ Architecture Documentation
**System architecture and boundaries**
- [architecture/ui-core-boundary.md](architecture/ui-core-boundary.md) - UI and Core boundary details
- [architecture/data-flow.md](architecture/data-flow.md) - Data flow diagrams

### ❓ Open Questions
**Unknown specifications and implementation decisions**
- [open-questions.md](open-questions.md) - Open questions and implementation guidance

### 📚 Legacy Technical Documentation
**Technical implementation details**
- [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) - Project overview and status
- [ARCHITECTURE.md](ARCHITECTURE.md) - Deep dive into system architecture
- [CODEBASE_STRUCTURE.md](CODEBASE_STRUCTURE.md) - Detailed codebase organization
- [API_REFERENCE.md](API_REFERENCE.md) - Complete API documentation
- [DEVELOPMENT_GUIDE.md](DEVELOPMENT_GUIDE.md) - Practical development guidance
- [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md) - Code conventions and idioms
- [PROJECT_QUIRKS.md](PROJECT_QUIRKS.md) - Project-specific quirks and implementation details
- [FUTURE_AND_MAINTAINABILITY.md](FUTURE_AND_MAINTAINABILITY.md) - Future roadmap and maintainability analysis
- [IMMEDIATE_IMPROVEMENTS.md](IMMEDIATE_IMPROVEMENTS.md) - Specific actionable improvements

## Quick Start for LLMs/Agents

### Understanding the Product
1. Read **product/overview.md** to understand what features are implemented
2. Review **product/concepts.md** to understand domain concepts
3. Study **product/feature-map.md** to understand feature relationships

### Understanding User Interactions
1. Read relevant **usecases/*.md** files to understand user interaction flows
2. Review **architecture/ui-core-boundary.md** to understand GUI/Core separation
3. Study **architecture/data-flow.md** to understand data transformations

### Working with the Codebase
1. Use **API_REFERENCE.md** to understand available functions and types
2. Follow **DEVELOPMENT_GUIDE.md** for coding patterns and conventions
3. Refer to **open-questions.md** for unknown specifications

### Common Tasks

**Adding a new feature:**
1. Check **product/feature-map.md** for dependencies
2. Review relevant **usecases/*.md** for similar patterns
3. Implement data structures in `lib/src/project/`
4. Implement logic in `core/src/`
5. Create Rust FFI wrappers in `guihlp/src/ffi/`
6. Create C++ FFI wrappers in `gui/src/ffi/`
7. Build UI in `gui/src/window/`
8. Follow naming conventions in CODE_CONVENTIONS.md

**Debugging an issue:**
1. Check network communication (core/src/network.rs, guihlp/src/network.rs)
2. Verify serialization (lib/src/requests.rs, lib/src/responces.rs)
3. Test FFI boundaries (guihlp/src/lib.rs, guihlp/src/ffi/)
4. Review state management (lib/src/lib.rs)
5. Be aware of project quirks in PROJECT_QUIRKS.md

**Understanding data flow:**
1. Review **architecture/data-flow.md** for detailed data flows
2. Check relevant **usecases/*.md** for specific operation flows
3. Note the dual runtime model and internal server pattern
4. Understand ChangeSet and incremental updates

**Planning improvements:**
1. Review **open-questions.md** for unknown specifications
2. Check technical debt analysis in FUTURE_AND_MAINTAINABILITY.md
3. Check immediate improvement recommendations in IMMEDIATE_IMPROVEMENTS.md
4. Refer to **product/feature-map.md** for feature dependencies

## Key Technologies

- **GUI**: Qt6 (C++)
- **Core**: Rust (2024 edition)
- **Graphics**: wgpu (WebGPU)
- **Video**: FFmpeg
- **Serialization**: rkyv
- **Async**: tokio
- **Build**: CMake + Cargo

## Project Components

### Components Overview
- **gui/**: Qt6 user interface
- **guihlp/**: Rust FFI bridge library
- **core/**: Rust core server application
- **lib/**: Shared Rust library

### Communication
- Custom binary protocol over TCP
- rkyv serialization for zero-copy deserialization
- Request/Response pattern for all operations
- Default port: 12345

## Important Notes

### Build System
- Uses CMake as primary build system
- Integrates Rust compilation via CMake
- Generates .clangd configuration for IDE support
- Supports Debug and Release builds

### Thread Safety
- Uses Arc<Mutex<T>> for shared state
- DashMap for concurrent hash maps
- Tokio runtime for async operations
- FFI callbacks for cross-language communication

### Error Handling
- Rust: Result<T, E> types
- C++: Exceptions and error codes
- FFI: WrapperErrorCode enum
- Comprehensive logging throughout

## Development Workflow

### Building
```bash
cmake -G Ninja -S '${workspaceFolder}' -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

### Testing
- Unit tests in Rust modules
- Integration tests for network communication
- Manual testing through GUI
- Performance profiling for rendering

### Debugging
- Rust: log:: macros
- C++: Qt logging (qDebug, qWarning)
- Network: Monitor TCP traffic
- FFI: Check error codes and callbacks

## Documentation Standards

### Code Documentation
- Public APIs documented with Rust doc comments
- Complex logic explained with comments
- FFI functions thoroughly documented
- Usage examples provided

### API Documentation
- Keep API reference current
- Document new functions and types
- Update architectural diagrams
- Maintain development guides

## Getting Help

### For Specific Issues
- Build problems: Check DEVELOPMENT_GUIDE.md troubleshooting section
- API questions: Refer to API_REFERENCE.md
- Architecture questions: Review ARCHITECTURE.md
- Code location: Check CODEBASE_STRUCTURE.md
- Code style and conventions: See CODE_CONVENTIONS.md
- Unusual behavior: Check PROJECT_QUIRKS.md
- Future planning: Review FUTURE_AND_MAINTAINABILITY.md
- Immediate improvements: Check IMMEDIATE_IMPROVEMENTS.md
- **Product understanding**: Check product/ directory
- **User interactions**: Check usecases/ directory
- **Unknown specs**: Check open-questions.md

### Understanding Context
- This is a hybrid C++/Rust project with Qt GUI
- Uses client-server architecture over TCP
- Video editing software with GPU rendering
- Currently under active development

## Future Development

### Planned Areas
- Enhanced audio processing
- More video effects and filters
- Export functionality
- Plugin system
- Cloud integration
- Performance optimizations

### Technical Debt
- Improve error handling consistency
- Increase test coverage
- Further code modularization
- Enhanced documentation

## File References

When referencing specific files in code or discussions, use the full path from the project root:
- `lib/src/project/clip.rs` - Clip data structures
- `lib/src/project/command.rs` - Command definitions (CommandRequest, CommandHistory)
- `core/src/network.rs` - Server network implementation
- `gui/src/main.cpp` - GUI entry point
- `guihlp/src/lib.rs` - FFI exports
- `guihlp/src/ffi/` - Rust FFI implementations
- `gui/src/ffi/` - C++ FFI implementations

## Contributing

When making changes:
1. Understand the existing architecture (see product/ and architecture/)
2. Follow code conventions (CODE_CONVENTIONS.md)
3. Update relevant documentation
4. Test across language boundaries
5. Verify network communication
6. Check for performance impacts
7. Document new use cases in usecases/
8. Update open-questions.md if specifications are clarified

---

This documentation is designed to help LLMs, Agents, and developers quickly understand and work with the Esotereel codebase. Start with product/overview.md and progress through the other documents as needed for specific tasks.
