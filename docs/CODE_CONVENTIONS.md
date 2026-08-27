# Esotereel Code Conventions and Idioms

This document describes the specific coding conventions, patterns, and idioms used throughout the Esotereel codebase. Understanding these conventions is essential for working effectively with the codebase.

## Rust Code Conventions

### Edition and Features
- **Rust Edition**: 2024 (latest features)
- **Async Runtime**: tokio with full features
- **FFI**: Extensive use of `extern "C"` for C interoperability

### Naming Conventions

#### Types and Structs
```rust
// Struct names: PascalCase
pub struct ProjectModel { }
pub struct ServerState { }
pub enum ClipData { }

// Field names: snake_case
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    next_clip_id: u64,
}
```

#### Functions and Methods
```rust
// Functions: snake_case
pub fn new_clip_in_timeline() -> EsotereelResult<ClipId> { }
pub fn render_frame_offscreen() -> Result<(), String> { }

// Methods: snake_case
pub fn get_timeline(&self, id: u64) -> Option<&TimelineModel> { }
pub fn timeline_mut(&mut self, id: TimelineId) -> Option<&mut Timeline> { }
```

#### Constants
```rust
// Constants: SCREAMING_SNAKE_CASE
pub const CLIENT_ALL: u32 = u32::MAX;
pub const NO_CLIENT: u32 = u32::MAX;
const AV_TIME_BASE: f64 = 1_000_000.0;
```

#### Type Aliases
```rust
// Type aliases: PascalCase for the alias, snake_case for module
pub type EsotereelResult<T> = Result<T, EsotereelError>;
pub type LayerMapKey = u64;
pub type ClipUpdateMap = HashMap<LayerMapKey, Vec<Clip>>;
```

### Memory Management Patterns

#### Shared State Pattern
```rust
// Arc<Mutex<T>> for exclusive access shared state
pub struct ServerState {
    pub project: Arc<RwLock<Option<Project>>>,
    pub next_resource_id: Arc<AtomicU32>,
}

// Arc<DashMap> for concurrent access
pub struct ClientState {
    pub path_to_stream: Arc<DashMap<String, StreamState>>,
    pub streams: Arc<DashMap<u32, StreamPlayer>>,
}
```

#### Thread Safety Implementation
```rust
// Manual Send/Sync implementations for FFI types
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
```

### Error Handling Patterns

#### Custom Result Types
```rust
// Domain-specific result type
pub type EsotereelResult<T> = Result<T, EsotereelError>;

// Custom error enum
pub enum EsotereelError {
    InvalidTimeline,
    LayerNotFound,
    DuplicateLayerOrder,
    AccessError(String),
}
```

#### Error Propagation
```rust
// Use ? operator for clean error propagation
pub fn new_clip_in(
    &mut self,
    layer_order: u32,
    position: i64,
    duration: i64,
    clip_data: ClipData,
    translates: ClipTranslates,
) -> Result<u64, EsotereelError> {
    let layer = self.layers
        .get_mut(&layer_order)
        .ok_or(EsotereelError::LayerNotFound)?;
    // ... rest of implementation
}
```

### Serialization Patterns

#### rkyv for Network Serialization
```rust
// All network types use rkyv attributes
#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ProjectModel {
    pub timelines: BTreeMap<u64, TimelineModel>,
    id_generator: IdGenerator,
}
```

#### Dual Serialization Support
```rust
// Many types support both rkyv (network) and serde (storage)
#[derive(Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Clip {
    pub id: u64,
    pub position: i64,
    pub duration: i64,
    pub data: ClipData,
    pub translates: ClipTranslates,
}
```

### FFI Patterns

#### C-Compatible Exports
```rust
// All FFI functions use unsafe and extern "C"
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}
```

#### Error Handling for FFI
```rust
// FFI-specific error codes
#[repr(C)]
pub enum WrapperErrorCode {
    Ok = 0,
    NullPtr = 1,
    NotFound = 2,
    Error = 3,
    Panic = 4,
}

// Thread-local error message storage
thread_local! {
    static LAST_ERR_MSG: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}
```

#### Pointer Safety
```rust
// Safe pointer wrapper function
pub fn slice_from_ptr_safe<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}
```

### Global State Pattern

#### OnceLock for Global Variables
```rust
// Global callbacks using OnceLock
pub(crate) static SEND_REQUEST_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();
pub(crate) static SEND_RESPONSE_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();

static GUI_CALLBACKS: OnceLock<GuiCallbacks> = OnceLock::new();
static ON_CONNECTED_CALLBACKS: OnceLock<OnConnectedFn> = OnceLock::new();
```

#### Global Instance Pattern
```rust
// Global instance for FFI callbacks
pub static INSTANCE: RwLock<Option<Arc<ServerNetworkHandler>>> = RwLock::new(None);

impl ServerNetworkHandler {
    pub fn get_instance() -> Option<Arc<ServerNetworkHandler>> {
        if let Ok(instance_guard) = INSTANCE.read() {
            if let Some(instance) = instance_guard.as_ref() {
                return Some(instance.clone());
            }
        }
        None
    }
}
```

### ID Management Pattern

#### Centralized ID Generation
```rust
// Single ID generator for all entity types
pub struct IdGenerator {
    next_timeline_id: u64,
    next_layer_id: u64,
    next_clip_id: u64,
}

impl IdGenerator {
    pub fn next_timeline_id(&mut self) -> u64 {
        let id = self.next_timeline_id;
        self.next_timeline_id += 1;
        id
    }
}
```

#### ID Observation for Deserialization
```rust
// Update ID generator when loading data
pub fn observe_timeline_id(&mut self, id: u64) {
    self.id_generator.observe_timeline(id);
}

pub fn observe_clip_id(&mut self, clip_id: u64) {
    self.next_clip_id = self.next_clip_id.max(clip_id + 1);
}
```

### Async Patterns

#### Tokio Runtime
```rust
// Main async entry point
#[tokio::main]
async fn main() {
    init_logger(log_out_callback);
    server_network_start("0.0.0.0:12345", None).await;
}
```

#### Channel-based Communication
```rust
// Unbounded channels for network communication
type ClientSender = mpsc::UnboundedSender<AlignedVec>;

let (tx, mut rx) = mpsc::unbounded_channel::<AlignedVec>();
```

#### Spawn Per-Client Tasks
```rust
// Each client gets its own tokio task
tokio::spawn(async move {
    log::info!("Client {} connected", client_id);
    // Client-specific handling
});
```

### Data Structure Patterns

#### BTreeMap for Ordered Data
```rust
// Use BTreeMap when order matters
pub struct ProjectModel {
    pub timelines: BTreeMap<u64, TimelineModel>, // Ordered by timeline ID
}

pub struct TimelineModel {
    pub layers: BTreeMap<u32, LayerModel>, // Ordered by layer order
}
```

#### HashMap for Unordered Lookups
```rust
// Use HashMap for unordered access
pub type ClipUpdateMap = HashMap<LayerMapKey, Vec<Clip>>;
```

### Logging Patterns

#### Log Level Usage
```rust
// Error: Critical failures
log::error!("Server failed to start: {}", e);

// Info: Important state changes
log::info!("Server listening on {}", addr);
log::info!("Client {} connected", client_id);

// Debug: Detailed debugging information
log::info!("fetch_stream_data called: seek_seconds={:?}", seek_sec_range);
```

#### Custom Logger Callback
```rust
// Custom logger initialization
pub fn init_logger(callback: fn(usize, String)) {
    // Initialize logger with callback
}
```

## C++ Code Conventions

### Language Standard
- **C++ Standard**: C++20
- **Compiler**: GCC/Clang on Linux, MSVC on Windows

### Naming Conventions

#### Classes and Structs
```cpp
// Classes: PascalCase
class MainWindow : public QMainWindow { };
class TimelineWidget : public QWidget { };
class ClientNetworkHandler { };

// Structs: PascalCase
struct WindowGState {
    ClientNetworkHandler *network;
    TimelineWidget *focusedTimeline = nullptr;
    CameraInfo *camera{};
};
```

#### Methods and Functions
```cpp
// Methods: camelCase
void markDirtyTimeline(size_t timelineId);
ClientNetworkHandler* network();
void onConnectedCallBack();

// Functions: camelCase
void bootcore(QString corePath);
void onServerStart(bool ok);
```

#### Variables
```cpp
// Local variables: camelCase
MainWindow *window;
QString addr;
```

#### Constants and Macros
```cpp
// Constants: SCREAMING_SNAKE_CASE or camelCase for Qt
const int MAX_CLIENTS = 100;
Qt::WindowFlags windowFlags;
```

### Pointer Style

#### Pointer Declaration
```cpp
// Pointer asterisk with type (preferred in this codebase)
ClientNetworkHandler *network;
TimelineWidget *focusedTimeline = nullptr;
CameraInfo *camera{};

// NOT: ClientNetworkHandler* network (not used in this codebase)
```

#### Null Pointer
```cpp
// Use nullptr for null pointers
TimelineWidget *focusedTimeline = nullptr;
CameraInfo *camera{};

// NOT: NULL or 0 (not used in this codebase)
```

### Qt Integration Patterns

#### Qt Object Management
```cpp
// Qt parent-child relationship for automatic memory management
TimelineWidget *timelineWidget = new TimelineWidget(this);
DebugStreamsWidget *debugStreamsWidget = new DebugStreamsWidget(this);
```

#### Signal-Slot Connections
```cpp
// Lambda-based signal-slot connections (modern style)
callbacks.mark_dirty_timeline = +[](size_t id) { window->markDirtyTimeline(id); };
callbacks.on_test = +[]() {};
```

#### Qt Logging
```cpp
// Use Qt logging categories
Q_LOGGING_CATEGORY(logRust, "lib")

// Use Qt logging functions
qDebug() << "Debug message:" << value;
qWarning() << "Warning:" << message;
qCritical() << "Error:" << error;
```

### FFI Integration Patterns

#### C-Compatible Callbacks
```cpp
// C-style callbacks for Rust integration
extern "C" void onConnectedCallBack();
extern "C" void mark_dirty_timeline(size_t id);

// Lambda wrappers for member functions
callbacks.mark_dirty_timeline = +[](size_t id) { window->markDirtyTimeline(id); };
```

#### Rust Function Calls
```cpp
// Call Rust FFI functions directly
esotereel_gui_helper::init();
esotereel_gui_helper::set_gui_callbacks(callbacks);
esotereel_gui_helper::set_on_connected_callback(onConnectedCallBack);
```

#### Error Handling from Rust
```cpp
// Check error codes from Rust FFI
auto error = esotereel_gui_helper::some_function();
if (error != esotereel_gui_helper::WrapperErrorCode::Ok) {
    const char* msg = esotereel_gui_helper::get_last_err_msg();
    // Handle error
}
```

### Memory Management

#### Smart Pointers
```cpp
// Use smart pointers where appropriate
std::shared_ptr<Project> project;
std::unique_ptr<RenderWorker> worker;

// Raw pointers for Qt objects (parent-child relationship)
TimelineWidget *timelineWidget; // Managed by Qt parent
```

#### RAII Pattern
```cpp
// Use RAII for resource management
{
    QFile file(path);
    if (file.open(QIODevice::ReadOnly)) {
        // File automatically closed when scope ends
    }
}
```

### String Handling

#### Qt Strings
```cpp
// Use QString for UI strings
QString filePath = "/path/to/file";
QString windowTitle = "Esotereel Video Editor";

// String conversion
std::string stdStr = qStr.toStdString();
QString qStr = QString::fromStdString(stdStr);
```

#### String Views
```cpp
// Use StringView for FFI string parameters
#include "wrapper/stringview.h"

// Avoid unnecessary string copies
void processString(StringView view);
```

### Header Organization

#### Header Guards
```cpp
#pragma once
// Include guards using #pragma once (modern style)

// Traditional guards also used in some files
#ifndef SOME_HEADER_H
#define SOME_HEADER_H
// ...
#endif
```

#### Forward Declarations
```cpp
// Use forward declarations to reduce compilation dependencies
class TimelineWidget;
class DebugStreamsWidget;
struct WindowGState;

// Forward declaration headers
#include "../wrapper/network.fwd.h"
#include "../wrapper/project/camera.fwd.h"
```

### Exception Handling

#### Exception Usage
```cpp
// Use exceptions for error handling in C++ code
try {
    // Some operation
} catch (const std::exception& e) {
    qCritical() << "Exception:" << e.what();
}
```

#### FFI Exception Safety
```cpp
// Exception handling at FFI boundaries
#include "wrapper/Result.h"

try {
    // Call Rust FFI
} catch (...) {
    // Convert to FFI error code
    return esotereel_gui_helper::WrapperErrorCode::Error;
}
```

## Cross-Language Patterns

### Data Structure Mapping

#### Rust Struct to C++ Class
```rust
// Rust
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
}
```

```cpp
// C++ wrapper
class ProjectWrapper {
public:
    // Wraps Rust Project
    // Provides C++ interface
};
```

#### Enum Mapping
```rust
// Rust enum
pub enum ClipData {
    Video { path: String, media_offset: f64 },
    Audio { path: String, media_offset: f64 },
}
```

```cpp
// C++ equivalent
enum class ClipDataType {
    Video,
    Audio,
};
```

### Error Code Consistency

#### Rust Error to C++ Error Code
```rust
// Rust
impl WrapperErrorCode {
    pub fn error(message: Option<&str>) -> Self {
        Self::set_last_err_msg(message);
        WrapperErrorCode::Error
    }
}
```

```cpp
// C++
if (rustResult != WrapperErrorCode::Ok) {
    const char* msg = get_last_err_msg();
    // Handle error
}
```

### Callback Patterns

#### Rust to Qt Callbacks
```rust
// Rust callback definition
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub mark_dirty_timeline: extern "C" fn(timeline_type: TimelineId),
}
```

```cpp
// C++ callback registration
esotereel_gui_helper::GuiCallbacks callbacks;
callbacks.mark_dirty_timeline = +[](size_t id) {
    window->markDirtyTimeline(id);
};
esotereel_gui_helper::set_gui_callbacks(callbacks);
```

## File Organization Patterns

### Module Structure

#### Rust Modules
```rust
// mod.rs exports module contents
pub mod clip;
pub mod camera;
pub mod commands;

// Use re-exports for public API
pub use {
    clip::Clip,
    runtime::{Project, timeline::Layer, timeline::Timeline},
};
```

#### C++ Headers
```cpp
// Forward declaration headers (.fwd.h)
#pragma once
class ClientNetworkHandler;

// Main headers
#pragma once
#include "network.fwd.h"
#include <QObject>

class ClientNetworkHandler : public QObject {
    // Implementation
};
```

### Include Order

#### Rust
```rust
// Standard library
use std::sync::{Arc, Mutex};

// External crates
use tokio::net::TcpListener;

// Internal modules
use crate::project::Project;
use crate::decode::VideoStreamer;
```

#### C++
```cpp
// System headers
#include <QObject>
#include <QString>

// Project headers
#include "network.fwd.h"
#include "project/project.h"

// Local headers
#include "window/main.h"
```

## Specific Code Idioms

### Builder Pattern in Rendering
```rust
// Vertex building for rendering
let vertices = build_vertices(timeline, app_state, current_frame);

let batches: Vec<RenderBatch> = vertices
    .into_iter()
    .map(|b| {
        let bind_group = util.textures.get(&b.texture_id)
            .map(|(_, bg)| bg.clone())
            .unwrap_or_else(|| util.resources.dummy_bind_group.clone());
        
        RenderBatch {
            vertices: b.vertices,
            texture_bind_group: bind_group,
            transform: b.transform,
        }
    })
    .collect();
```

### Command Pattern for Operations
```rust
// Commands represent atomic operations
pub enum Command {
    AddClip { /* parameters */ },
    RemoveClip { /* parameters */ },
    ModifyClip { /* parameters */ },
}

// Command execution with history
pub fn execute_command(command: Command, project: &mut Project) -> EsotereelResult<()> {
    // Execute command
    // Update history
    // Generate updates for clients
}
```

### Option/Result Chaining
```rust
// Heavy use of Option and Result chaining
let timeline = self.timelines.get(&timeline_id)
    .ok_or(EsotereelError::InvalidTimeline)?;

let layer = timeline.get_layer_mut(layer_key)
    .ok_or(EsotereelError::LayerNotFound)?;
```

### Closure-Based Callbacks
```cpp
// Modern C++ lambda callbacks
callbacks.mark_dirty_timeline = +[](size_t id) {
    window->markDirtyTimeline(id);
};

// Rust closure-like patterns
map.values()
    .flat_map(|layer| layer.clips.values())
    .map(|clip| clip.id)
    .max()
```

## Performance-Related Patterns

### Zero-Copy Patterns
```rust
// Use rkyv for zero-copy deserialization
let archived = check_archived_root::<Request>(bytes)?;
// Work directly with archived data without copying

// Use slices instead of owned data when possible
pub fn slice_from_ptr_safe<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}
```

### Lazy Initialization
```rust
// Use OnceLock for lazy initialization
static GUI_CALLBACKS: OnceLock<GuiCallbacks> = OnceLock::new();

pub fn mark_dirty_timeline(timeline_type: TimelineId) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.mark_dirty_timeline)(timeline_type);
    }
}
```

### Resource Pooling
```rust
// Reuse allocations where possible
let mut rgb_frame = Video::new(Pixel::RGBA, width, height);
// Reuse frame in loop instead of reallocating
```

## Testing Patterns

### Unit Test Structure
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_creation() {
        let clip = Clip::new(/* parameters */);
        assert_eq!(clip.id(), expected_id);
    }

    #[test]
    fn test_timeline_operations() {
        let mut timeline = Timeline::new(0, 30.0);
        // Test timeline operations
    }
}
```

## Documentation Patterns

### Rust Documentation
```rust
/// Creates a new clip in the specified timeline and layer.
///
/// # Arguments
/// * `timeline_id` - The ID of the target timeline
/// * `layer_key` - The key of the target layer
/// * `position` - The position in frames
/// * `duration` - The duration in frames
///
/// # Returns
/// The ID of the created clip
///
/// # Errors
/// Returns an error if the timeline or layer doesn't exist
pub fn new_clip_in_timeline(/* parameters */) -> EsotereelResult<ClipId> {
    // Implementation
}
```

### C++ Documentation
```cpp
/// Main application window using Qt-Advanced-Docking-System
/// 
/// This class manages the main window and coordinates between
/// different UI components like timeline, preview, and debug windows.
class MainWindow : public QMainWindow {
    Q_OBJECT
public:
    /// Constructor that initializes the main window
    /// @param network Reference to the network handler
    /// @param parent Parent widget (optional)
    explicit MainWindow(ClientNetworkHandler &network, QWidget *parent = nullptr);
    
    /// Marks a timeline as dirty, triggering a redraw
    /// @param timelineId The ID of the timeline to mark dirty
    void markDirtyTimeline(size_t timelineId);
};
```

## Anti-Patterns to Avoid

### Rust Anti-Patterns
```rust
// DON'T: Use unwrap() in production code
let value = some_operation().unwrap(); // Avoid

// DO: Use proper error handling
let value = some_operation()?; // Better

// DON'T: Use unsafe without good reason
unsafe { /* arbitrary unsafe code */ } // Avoid

// DO: Document and justify unsafe usage
// SAFETY: This is safe because the pointer is guaranteed to be valid
unsafe { /* well-justified unsafe code */ } // Better
```

### C++ Anti-Patterns
```cpp
// DON'T: Use raw pointers for ownership
SomeClass* obj = new SomeClass(); // Avoid
delete obj; // Easy to forget

// DO: Use smart pointers
std::unique_ptr<SomeClass> obj = std::make_unique<SomeClass>(); // Better

// DON'T: Use NULL
SomeClass* ptr = NULL; // Avoid

// DO: Use nullptr
SomeClass* ptr = nullptr; // Better
```

These conventions and patterns reflect the specific idioms and practices used in the Esotereel codebase. Following them will help maintain consistency and make the codebase easier to understand and maintain.
