# Esotereel

Esotereel is a video editing application with a client-server architecture.

I use AI agents to fix bugs and write tests.

###### ⚠️ This project is under constructing!

## Build

```bash
cmake -G Ninja -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build
```

## TODO

### High Priority
- use macro or function to construct PropertySchema in core_settings_schema()
- Project save/load
- Export functionality (FFmpeg integration)
- Error handling UI
- Clip split
- Text clip
- Audio
- Media management (relocation, missing files)

### Medium Priority
- Network conflict handling
- Plugin security
- LOD clip view
- full plugin API for Requests/Responces and Commands 
- Timeline toolbar button icons (gui/src/window/widget/timeline/TimelineToolbarWidget.cpp:56)

### Low Priority
- Variable and Formula
- Scripting on editor
- Nested Timeline rendering


See [Clip Properties](docs/architecture/clip-properties.md) for shared property definitions and video transforms.
See [Open Questions](docs/open-questions.md) for detailed specifications.
See [Documentation Style Guide](docs/style-guide.md) for documentation standards.
See [AGENTS.md](AGENTS.md) for AI agent behavior guidelines.
