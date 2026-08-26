# Esotereel API Reference

This document provides detailed information about the key APIs and interfaces in the Esotereel codebase, intended for LLMs and Agents working with the codebase.

## Core Data Types

### Project Types

#### `Project` (lib/src/project/project.rs)
Runtime project structure used during application execution.

```rust
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
}
```

**Key Methods:**
- `new()` - Create empty project
- `insert_timeline(fps)` - Add new timeline with specified FPS
- `timeline(id)` - Get timeline by ID
- `timeline_mut(id)` - Get mutable timeline by ID
- `new_clip_in_timeline(...)` - Add clip to timeline layer
- `make_independent(source)` - Deep clone timeline for independent use
- `remove_timeline(id)` - Remove timeline
- `drain_changes()` - Collect pending changes from all timelines
- `propagate_nested_dirty(changed)` - Propagate changes to parent timelines
- `timelines_meta()` - Get lightweight metadata for ProjectAll sync
- `from_meta(timelines)` - Create project from metadata

#### `Timeline` (lib/src/project/timeline.rs)
Runtime timeline with layer hierarchy and clip storage.

```rust
pub struct Timeline {
    pub id: u64,
    pub fps: f64,
    root_layers: Vec<LayerId>,
    layers: HashMap<LayerId, Layer>,
    clips: HashMap<ClipId, Clip>,
    chunk_index: RwLock<Option<ChunkIndex>>,
    pending: ChangeSet,
}
```

**Key Methods:**
- `new(id, fps)` - Create timeline with default 4 layers
- `insert_layer(layer, parent, index)` - Add layer with hierarchy support
- `get_layer(id)` - Get layer by ID
- `get_layer_mut(id)` - Get mutable layer by ID
- `reorder_child(parent, id, new_index)` - Reorder layer in hierarchy
- `iter_execution_order()` - Flatten layer hierarchy for execution
- `new_clip_in(...)` - Add clip to layer with overlap detection
- `remove_clip_by_id(clip_id)` - Remove clip and return it
- `place_clip(layer_id, clip)` - Place existing clip at position
- `get_clip(id)` - Get clip by ID
- `get_clip_mut(id)` - Get mutable clip by ID
- `move_clip(clip_id, new_position)` - Move clip to new position
- `query_range(range)` - Query clips in time range using ChunkIndex
- `drain_changes()` - Get pending changes
- `has_pending_changes()` - Check if changes exist
- `from_meta(meta)` - Create timeline from metadata
- `merge_fetched_clips(entries)` - Merge fetched clips from network
- `upsert_clip_from_network(layer_id, clip)` - Upsert clip from network sync

#### `Layer` (lib/src/project/layer.rs)
Layer with folder support and clip position tracking.

```rust
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub enabled: bool,
    pub parent: Option<LayerId>,
    pub children: Vec<LayerId>,
    pub clips: BTreeMap<i64, ClipId>,
}
```

**Key Methods:**
- `new(id, name)` - Create layer
- `is_folder()` - Check if layer is a folder (has children)
- `get_clip_id_at(pos)` - Get clip ID at position
- `remove_clip(clip_id)` - Remove clip reference from layer

### Clip Types

#### `Clip` (lib/src/project/clip.rs)
Fundamental media unit in the timeline.

```rust
pub struct Clip {
    pub id: u64,
    pub position: i64,
    pub duration: i64,
    pub data: ClipData,
    pub translates: ClipTranslates,
}
```

**Key Methods:**
- `new(id, position, duration, data, translates)` - Create clip
- `position()` - Get clip position
- `set_position(new_pos)` - Set clip position

#### `ClipData` (lib/src/project/clip.rs)
Enum representing different types of media content.

```rust
pub enum ClipData {
    Dummy,
    Video { path: String, media_offset: f64 },
    Audio { path: String, media_offset: f64 },
    Composite { timeline_id: Option<TimelineId> },
    Area2D { timeline_id: Option<TimelineId> },
    Area3D { timeline_id: Option<TimelineId> },
}
```

**Key Methods:**
- `get_media_seconds(global_fps, clip_position, current_frame, media_offset)` - Calculate media time

## Network API

### Request Types (lib/src/requests.rs)

#### `Request` Enum
All client-to-server messages.

```rust
pub enum Request {
    Test,
    NewProject,
    ProjectAll,
    Command {
        command: Command,
        timeline_map_key: LayerMapKey,
    },
    InitStream {
        path: String,
    },
    FetchStreamData {
        resource_id: u32,
        seek_range_sec: Range<f64>,
    },
}
```

### Response Types (lib/src/responces.rs)

#### `Response` Enum
All server-to-client messages.

```rust
pub enum Response {
    Test,
    ProjectAll {
        project: ProjectModel,
    },
    ClipUpdates {
        timeline_map_key: LayerMapKey,
        updates: ClipUpdateMap,
    },
    StreamMetadata {
        path: String,
        resource_id: u32,
        codec_id: u16,
        width: u32,
        height: u32,
        time_base: f64,
        extradata: Vec<u8>,
    },
    StreamData {
        resource_id: u32,
        data: Vec<u8>,
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
    },
    StreamDataEnd {
        resource_id: u32,
        fetched_range: Range<f64>,
    },
}
```

## Network Handler API

### Server Network Handler (core/src/network.rs)

#### `ServerNetworkHandler`
Manages TCP server and client connections.

```rust
pub struct ServerNetworkHandler {
    pub app_state: Arc<Mutex<ServerState>>,
    clients: RwLock<HashMap<u32, ClientSender>>,
}
```

**Key Methods:**
- `new(app_state)` - Create handler with state
- `run(addr, on_server_ready)` - Start server (async)
- `send(client_id, response)` - Send response to specific client
- `send_all(response)` - Broadcast response to all clients
- `get_instance()` - Get global instance (for FFI callbacks)

**Static Methods:**
- `INSTANCE` - Global RwLock containing optional instance

### Client Network Handler (guihlp/src/network.rs)

#### `ClientNetworkHandler`
Manages TCP client connection to server.

```rust
pub struct ClientNetworkHandler {
    pub app_state: Arc<Mutex<ClientState>>,
    tx: RwLock<Option<ClientSender>>,
}
```

**Key Methods:**
- `new(app_state)` - Create handler with state
- `run(addr)` - Connect to server and start processing (async)
- `send(request)` - Send request to server
- `get_instance()` - Get global instance (for FFI callbacks)

## State Management API

### Server State (lib/src/lib.rs)

#### `ServerState`
Server-side application state.

```rust
pub struct ServerState {
    pub project: Arc<RwLock<Option<Project>>>,
    pub path_to_stream: Arc<DashMap<String, StreamState>>,
    pub streams: Arc<DashMap<u32, VideoStreamer>>,
    pub next_resource_id: Arc<AtomicU32>,
}
```

**Key Methods:**
- `new()` - Create empty server state
- `get_or_create_resource_id(path)` - Get or create resource ID for path

### Client State (lib/src/lib.rs)

#### `ClientState`
Client-side application state.

```rust
pub struct ClientState {
    pub project: Option<Arc<RwLock<Project>>>,
    pub path_to_stream: Arc<DashMap<String, StreamState>>,
    pub streams: Arc<DashMap<u32, StreamPlayer>>,
}
```

**Key Methods:**
- `new()` - Create empty client state

## Video Processing API

### Video Streamer (lib/src/decode/videostreamer.rs)

#### `VideoStreamer`
Server-side video decoding using FFmpeg.

```rust
pub struct VideoStreamer {
    pub ictx: Input,
    decoder: VideoDecoder,
    scaler: Scaler,
    pub video_stream_index: usize,
    pub time_base: f64,
    pub last_pts: Option<i64>,
    needs_discontinuity_flag: bool,
}
```

**Key Methods:**
- `new(path)` - Create streamer for video file
- `next_frame()` - Decode next frame
- `get_frame_at_time(seconds)` - Seek to specific time and decode frame
- `get_init_packet(path, resource_id)` - Get stream metadata as Response
- `fetch_stream_data(resource_id, range)` - Fetch video packets for time range
- `seek(seconds)` - Seek to specific time
- `codec_id()` - Get codec ID
- `width()` - Get video width
- `height()` - Get video height
- `extradata()` - Get codec extradata

### Stream Player (lib/src/decode/streamplayer.rs)

#### `StreamPlayer`
Client-side video playback management.

**Key Methods:**
- Video frame caching and management
- Playback control
- Frame retrieval for rendering

## Rendering API

### Main Render Function (lib/src/render/mod.rs)

#### `render_frame_offscreen`
Main rendering function for timeline preview.

```rust
pub fn render_frame_offscreen(
    util: &mut WGpuUtil,
    offscreen: &OffscreenTarget,
    timeline: &Timeline,
    app_state: &ClientState,
    camera_info: &CameraInfo,
    current_frame: i64,
) -> Result<(), String>
```

**Process:**
1. Calculate view/projection matrices
2. Build vertices from timeline state
3. Create render batches grouped by texture
4. Update GPU buffers
5. Update video textures
6. Execute render pass
7. Copy texture to buffer

### WGpuUtil (lib/src/render/wgpuutil.rs)

#### `WGpuUtil`
WebGPU utility for device and resource management.

**Key Components:**
- Device and queue management
- Buffer allocation and updates
- Texture management
- Render pipeline creation
- Bind group management

## FFI API

### GUI Helper Exports (guihlp/src/lib.rs)

#### C-Compatible Functions
Exported functions for Qt GUI to call.

**Initialization:**
- `init()` - Initialize library
- `set_gui_callbacks(callbacks)` - Set GUI callback functions
- `set_on_connected_callback(callback)` - Set connection callback

**Error Handling:**
- `get_last_err_msg()` - Get last error message (C string)
- `WrapperErrorCode` enum for error codes

**Callbacks:**
```rust
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub mark_dirty_timeline: extern "C" fn(timeline_type: TimelineId),
}
```

### Project Wrapper API (guihlp/src/wrapper/project/)

#### Project Operations
- Project creation and management
- Timeline operations
- Layer manipulation
- Clip management

#### Timeline Operations
- Timeline creation with hierarchy support
- Layer management with folder support
- FPS control
- Range queries using ChunkIndex

#### Clip Operations
- Clip creation with overlap detection
- Position and duration modification
- Transform operations
- Change tracking via ChangeSet

## Change Tracking API

### ChangeSet (lib/src/project/change.rs)

Change tracking for network synchronization.

```rust
pub struct ChangeSet {
    pub upserted: HashSet<ClipId>,
    pub removed: HashMap<ClipId, RemovedClipInfo>,
}

pub struct RemovedClipInfo {
    pub layer_id: LayerId,
    pub position: i64,
    pub duration: i64,
}
```

**Key Methods:**
- `is_empty()` - Check if changes exist
- `mark_upserted(id)` - Mark clip as upserted
- `mark_removed(id, info)` - Mark clip as removed
- `merge(other)` - Merge another ChangeSet

### ChunkIndex (lib/src/project/chunk_index.rs)

Spatial index for efficient time-range queries.

```rust
pub struct ChunkIndex {
    map: BTreeMap<ChunkId, Vec<(LayerId, ClipId)>>,
}
```

**Key Methods:**
- `build(entries)` - Build index from clip entries
- `candidates(range)` - Get candidate clips for time range

**Constants:**
- `CHUNK_TICKS: i64 = 30_000` - Size of each time chunk

## Command Pattern API

### Command Types (lib/src/project/commands.rs)

Commands represent atomic operations on the project state.

**Common Commands:**
- Add/remove clips
- Modify clip properties
- Layer operations
- Timeline operations

**Command Execution:**
- Commands are executed on the server
- Generate incremental updates for clients
- Support undo/redo through history system

## Utility API

### ID Management (lib/src/project/ids.rs)

#### `IdGenerator`
Manages unique ID generation for project entities.

```rust
pub struct IdGenerator {
    next_timeline_id: u64,
    next_layer_id: u64,
    next_clip_id: u64,
}
```

**Key Methods:**
- `next_timeline_id()` - Generate next timeline ID
- `next_layer_id()` - Generate next layer ID
- `observe_timeline(id)` - Update generator from loaded data

### Result Types (lib/src/util/result.rs)

#### `EsotereelResult`
Common result type for error handling.

```rust
pub type EsotereelResult<T> = Result<T, EsotereelError>;
```

#### `EsotereelError`
Domain-specific error types.

```rust
pub enum EsotereelError {
    InvalidTimeline,
    LayerNotFound,
    DuplicateLayerOrder,
    AccessError(String),
    // ... other error types
}
```

## Camera API

### Camera Info (lib/src/project/camera.rs)

#### `CameraInfo`
Camera parameters for rendering.

**Key Methods:**
- `get_proj_mat(screen_size)` - Get projection matrix
- `get_view_mat()` - Get view matrix
- Camera position and orientation management

## Transform API

### Clip Translates (lib/src/project/transform.rs)

#### `ClipTranslates`
Transform data for clips.

**Components:**
- Position (translation)
- Rotation
- Scale
- Other transform properties

## Serialization API

### rkyv Integration
Most data structures support rkyv serialization for network transmission.

**Attributes:**
```rust
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize)]
#[archive_attr(derive(CheckBytes))]
```

**Usage:**
- `rkyv::to_bytes::<_, 1024>(data)` - Serialize to bytes
- `check_archived_root::<Type>(bytes)` - Deserialize and validate

### serde Integration
Additional serde support for JSON serialization.

**Attributes:**
```rust
#[derive(serde::Serialize, serde::Deserialize)]
```

## Thread Safety

### Arc<Mutex<T>> Pattern
Shared state across threads using Arc and Mutex.

**Example:**
```rust
pub project: Arc<RwLock<Option<Project>>>,
```

### DashMap
Concurrent hash map for high-performance concurrent access.

**Usage:**
```rust
pub path_to_stream: Arc<DashMap<String, StreamState>>,
```

## Callback API

### Rust to Qt Callbacks
FFI callbacks for Rust to notify Qt of state changes.

**Callback Registration:**
```rust
pub fn set_gui_callbacks(callbacks: GuiCallbacks)
```

**Callback Invocation:**
```rust
pub fn mark_dirty_timeline(timeline_type: TimelineId) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.mark_dirty_timeline)(timeline_type);
    }
}
```

## Network Protocol

### Message Format
```
[4 bytes: length (little-endian)][N bytes: rkyv-serialized data]
```

### Connection Flow
1. Server starts listening on configured port
2. Client connects to server
3. Client sends requests
4. Server processes and sends responses
5. Connection remains open for continued communication

## Constants

### Network Constants
```rust
pub const CLIENT_ALL: u32 = u32::MAX;  // Broadcast to all clients
pub const NO_CLIENT: u32 = u32::MAX;   // No specific client
```

### Video Constants
```rust
const AV_TIME_BASE: f64 = 1_000_000.0;  // FFmpeg time base
```

### Timeline Constants
```rust
pub const MAX_NESTED_DEPTH: u32 = 32;  // Maximum nesting depth for composite/mirror
```

## Logging API

### Logger Initialization (lib/src/util/logger.rs)

#### `init_logger`
Initialize logging system with callback.

```rust
pub fn init_logger(callback: fn(usize, String))
```

**Log Levels:**
- 1: ERROR
- 2: WARN
- 3: INFO
- 4: DEBUG
- 5: TRACE

## Integration Points

### Adding New Request Types
1. Add variant to `Request` enum in `lib/src/requests.rs`
2. Add handler in `core/src/requests.rs`
3. Add FFI wrapper in `guihlp/src/wrapper/requests.rs`
4. Add C++ wrapper in `gui/src/wrapper/requests.h/cpp`

### Adding New Response Types
1. Add variant to `Response` enum in `lib/src/responces.rs`
2. Add handler in `guihlp/src/responces.rs`
3. Add Qt handler in appropriate GUI component

### Adding New Clip Data Types
1. Add variant to `ClipData` enum in `lib/src/project/clip.rs`
2. Implement rendering logic in `lib/src/render/`
3. Add UI components in `gui/src/`
4. Update serialization if needed

This API reference provides the essential interfaces needed to understand and work with the Esotereel codebase. For specific implementation details, refer to the individual source files.
