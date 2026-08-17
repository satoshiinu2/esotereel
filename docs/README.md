# Esotereel Documentation

This directory contains comprehensive documentation for the Esotereel video editing software project, designed for LLMs, Agents, and developers working with the codebase.

## Documentation Structure

### 📋 [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md)
**Start here for an introduction to the project**
- Project description and status
- Key technologies and architecture overview
- Project structure and build system
- Communication protocol and key features
- Development environment setup

### 🏗️ [ARCHITECTURE.md](ARCHITECTURE.md)
**Deep dive into system architecture**
- Architectural layers and component responsibilities
- Communication protocol and data flow
- Data architecture (project models, rendering, video processing)
- Component interactions and startup sequence
- Technology rationale and design decisions
- Concurrency model and error handling
- Performance considerations and security

### 📁 [CODEBASE_STRUCTURE.md](CODEBASE_STRUCTURE.md)
**Detailed codebase organization**
- Directory structure for all components
- Key files and their purposes
- Data flow between components
- Build artifacts and configuration files
- Development workflow and dependencies

### 🔌 [API_REFERENCE.md](API_REFERENCE.md)
**Complete API documentation**
- Core data types (Project, Timeline, Clip)
- Network API (Request/Response types)
- Network handler APIs
- State management APIs
- Video processing APIs
- Rendering APIs
- FFI APIs and callbacks
- Utility APIs and constants

### 🛠️ [DEVELOPMENT_GUIDE.md](DEVELOPMENT_GUIDE.md)
**Practical development guidance**
- Build instructions and prerequisites
- Development workflow and patterns
- Testing and debugging strategies
- Performance optimization tips
- Common issues and solutions
- Best practices and collaboration guidelines

### 📝 [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)
**Code conventions and idioms**
- Rust code conventions and patterns
- C++ code conventions and patterns
- Cross-language integration patterns
- File organization and structure
- Specific code idioms used in the codebase
- Performance-related patterns
- Anti-patterns to avoid

### 🎯 [PROJECT_QUIRKS.md](PROJECT_QUIRKS.md)
**Project-specific quirks and implementation details**
- Architecture quirks and design decisions
- Data structure peculiarities
- Network and FFI implementation details
- Rendering and FFmpeg integration specifics
- Build system and dependency quirks
- Performance characteristics and limitations

### 🚀 [FUTURE_AND_MAINTAINABILITY.md](FUTURE_AND_MAINTAINABILITY.md)
**Future roadmap and maintainability analysis**
- Feature roadmap (short, medium, long-term)
- Technical debt analysis and prioritization
- Scalability analysis and improvements
- Maintainability improvements and refactoring opportunities
- Security considerations and improvements
- Risk assessment and mitigation strategies

### ⚡ [IMMEDIATE_IMPROVEMENTS.md](IMMEDIATE_IMPROVEMENTS.md)
**Specific actionable improvements for immediate implementation**
- Critical safety improvements (unsafe code documentation, FFI validation)
- Error handling standardization
- Testing infrastructure setup
- Documentation improvements
- Code quality enhancements
- Performance monitoring setup
- 12-week implementation timeline

## Quick Start for LLMs/Agents

### Understanding the Project
1. Read **PROJECT_OVERVIEW.md** to understand what this project is
2. Review **ARCHITECTURE.md** to understand how components interact
3. Study **CODEBASE_STRUCTURE.md** to know where code is located

### Working with the Codebase
1. Use **API_REFERENCE.md** to understand available functions and types
2. Follow **DEVELOPMENT_GUIDE.md** for coding patterns and conventions
3. Refer to specific source files for implementation details

### Common Tasks

**Adding a new feature:**
- Start with data structures in `lib/src/project/`
- Implement logic in `core/src/`
- Create FFI wrappers in `guihlp/src/`
- Build UI in `gui/src/`
- Follow naming conventions in CODE_CONVENTIONS.md

**Debugging an issue:**
- Check network communication (core/src/network.rs, guihlp/src/network.rs)
- Verify serialization (lib/src/requests.rs, lib/src/responces.rs)
- Test FFI boundaries (guihlp/src/lib.rs)
- Review state management (lib/src/lib.rs)
- Be aware of project quirks in PROJECT_QUIRKS.md

**Understanding data flow:**
- Request flow: GUI → C++ → Rust FFI → Network → Core
- Response flow: Core → Network → Rust FFI → C++ → GUI
- Data structures use rkyv for zero-copy serialization
- Note the dual runtime model and internal server pattern

**Planning improvements:**
- Review technical debt analysis in FUTURE_AND_MAINTAINABILITY.md
- Check immediate improvement recommendations in IMMEDIATE_IMPROVEMENTS.md
- Prioritize based on impact and effort estimates
- Follow 12-week implementation timeline for systematic improvements

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
- Rust: log:: macros and println!
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
- `core/src/network.rs` - Server network implementation
- `gui/src/main.cpp` - GUI entry point
- `guihlp/src/lib.rs` - FFI exports

## Contributing

When making changes:
1. Understand the existing architecture
2. Follow code conventions
3. Update relevant documentation
4. Test across language boundaries
5. Verify network communication
6. Check for performance impacts

---

This documentation is designed to help LLMs, Agents, and developers quickly understand and work with the Esotereel codebase. Start with PROJECT_OVERVIEW.md and progress through the other documents as needed for specific tasks.
