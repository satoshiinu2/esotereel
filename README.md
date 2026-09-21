# Esotereel

Esotereel is a video editing application with a client-server architecture.

###### ⚠️ This project is under constructing!

## Build

```bash
cmake -G Ninja -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build
```

## TODO

### High Priority
- Undo/Redo functionality
- Project save/load
- Export functionality (FFmpeg integration)
- Error handling UI

### Medium Priority
- Media management (relocation, missing files)
- Network conflict handling
- Plugin security
- LOD clip view

### Low Priority
- Scripting on editor
- Nested Timeline rendering

### Code TODOs
- `ClipTranslates::Keyframe` implementation (lib/src/project/transform.rs:31)
- Settings loading (lib/src/plugin/settings/mod.rs:124)
- Timeline toolbar button icons (gui/src/window/widget/timeline/TimelineToolbarWidget.cpp:56)
- Deprecated API replacements (guihlp/src/lib.rs)

See [Open Questions](docs/open-questions.md) for detailed specifications.
