# Esotereel

Esotereel is a video editing application with a client-server architecture.

###### ⚠️ This project is under constructing!

## Build

```bash
cmake -G Ninja -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build
```

## TODO

### High Priority
- in core/src/project/commands.rs: make anything instead of hardcode source_offset
- Project save/load
- Export functionality (FFmpeg integration)
- Error handling UI

### Medium Priority
- Media management (relocation, missing files)
- Network conflict handling
- Plugin security
- LOD clip view
- full plugin API for Requests/Responces and Commands 

### Low Priority
- Scripting on editor
- Nested Timeline rendering

### Code TODOs
- `ClipTranslates::Keyframe` implementation (lib/src/project/transform.rs:31)
- Timeline toolbar button icons (gui/src/window/widget/timeline/TimelineToolbarWidget.cpp:56)

See [Clip Properties](docs/architecture/clip-properties.md) for shared property definitions and video transforms.
See [Open Questions](docs/open-questions.md) for detailed specifications.
See [Documentation Style Guide](docs/style-guide.md) for documentation standards.
See [AGENTS.md](AGENTS.md) for AI agent behavior guidelines.
