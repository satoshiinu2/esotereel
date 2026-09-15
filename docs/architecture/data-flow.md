# データフロー

## 重要な操作のデータフロー

### 1. プロジェクト作成フロー

```
User Action (NewProject)
    ↓
GUI: Menu Click
    ↓
GUI State: Project Unloaded
    ↓
C++ FFI: Request Creation
    ↓
Rust FFI: Request::NewProject Serialization
    ↓
Network: TCP Send
    ↓
Core: Request Handler
    ↓
Domain Logic:
  - Project::new()
  - Project::insert_timeline(60.0)
  - Timeline::new() → 4 Layer Created
  - Project::timelines_meta()
    ↓
State Mutation:
  - ServerState.project = Arc::new(RwLock::new(project))
  - ServerState.network.client_views[client_id] = {timeline_id: full_range}
    ↓
Response: Response::ProjectMeta { timelines }
    ↓
Network: TCP Send
    ↓
GUI: Response Handler
    ↓
C++: Timeline::from_meta() → Skeleton Timeline Created
    ↓
GUI State: Timeline UI Displayed
    ↓
Result: New Project with Default Timeline
```

**Data Transformations**:
- Domain Logic: Project → TimelineMeta
- Network: TimelineMeta → rkyv bytes
- GUI: rkyv bytes → TimelineMeta → Timeline skeleton

---

### 2. クリップ追加フロー

```
User Action (AddClip)
    ↓
GUI: File Drop
    ↓
GUI State: File Path, Drop Position
    ↓
GUI Processing:
  - Calculate position (TimelineTick)
  - Determine duration (Unknown)
  - Select layer_id
  - Determine kind_id (Plugin ClipKind)
  - Build properties map
  - Build translates
    ↓
C++ FFI: Command Request Creation
    ↓
Rust FFI: Request::Command { CommandRequest::AddClip } Serialization
    ↓
Network: TCP Send
    ↓
Core: Request Handler
    ↓
Domain Logic:
  - command_to_history() → CommandHistory::AddClip
  - handle_command_action()
  - clip_add_core()
  - Timeline::new_clip_in()
    - IdGenerator::next_clip_id()
    - Clip::new()
    - Layer.clips.insert(position, clip_id)
    - Timeline.clips.insert(clip_id, clip)
    - Timeline::touch_upsert(clip_id)
    - Timeline::invalidate_index()
    ↓
State Mutation:
  - Timeline.clips: {clip_id: Clip}
  - Layer.clips: {position: clip_id}
  - Timeline.changes.clips_upserted: {clip_id}
  - Timeline.chunk_index: None (invalidated)
    ↓
Dirty Notification: notify_dirty()
    ↓
Change Sync:
  - Project::drain_changes() → ChangeSet
  - Response::UpdateClip { timeline_id, clips: [(layer_id, clip)] }
    ↓
Network: TCP Send
    ↓
GUI: Response Handler
    ↓
C++: Timeline::upsert_clip_from_network()
  - Remove old references from all layers
  - Add reference to target layer
  - Update Timeline.clips
  - Timeline::invalidate_index()
    ↓
GUI State: Timeline UI Updated
    ↓
Result: Clip Added to Timeline
```

**Data Transformations**:
- GUI: File path → Clip parameters
- Domain Logic: Parameters → Clip entity
- Network: Clip → rkyv bytes
- GUI: rkyv bytes → Clip entity → UI representation

**Data Generated**:
- ClipId (new)
- Clip entity
- ChangeSet entry

**Data Modified**:
- Timeline.clips
- Layer.clips
- Timeline.changes
- Timeline.chunk_index

---

### 3. クリップ移動フロー

```
User Action (ClipsMove)
    ↓
GUI: Drag Operation
    ↓
GUI State: Selected Clips, Current Position
    ↓
GUI Processing:
  - Track drag position
  - Calculate new_position
  - Calculate new_duration (if resizing)
  - Determine new_layer_id
  - Build Vec<ClipMoveCtx>
    ↓
C++ FFI: Command Request Creation
    ↓
Rust FFI: Request::Command { CommandRequest::ClipsMove } Serialization
    ↓
Network: TCP Send
    ↓
Core: Request Handler
    ↓
Domain Logic:
  - command_to_history()
    - Timeline::get_clip_and_layer() for each clip
    - Build ClipMoveHistoryCtx (old_*, new_*)
  - handle_command_action()
  - clip_move_mul_core()
  - For each ClipMoveHistoryCtx:
    - Timeline::get_clip_and_layer()
    - Timeline::remove_clip_by_id_in(old_layer_id, clip_id)
    - Update Clip (position, duration)
    - Timeline::place_clip(new_layer_id, updated_clip)
      - Remove from old layer
      - Add to new layer
      - Timeline::touch_upsert(clip_id)
    ↓
State Mutation:
  - Timeline.clips: {clip_id: updated_clip}
  - Layer.clips (old): remove {position: clip_id}
  - Layer.clips (new): add {new_position: clip_id}
  - Timeline.changes.clips_upserted: {clip_id, ...}
  - Timeline.chunk_index: None (invalidated)
    ↓
Dirty Notification: notify_dirty()
    ↓
Change Sync:
  - Project::drain_changes() → ChangeSet
  - Response::UpdateClip { timeline_id, clips: [(new_layer_id, updated_clip), ...] }
    ↓
Network: TCP Send
    ↓
GUI: Response Handler
    ↓
C++: Timeline::upsert_clip_from_network() for each clip
  - Remove old references from all layers
  - Add reference to new layer
  - Update Timeline.clips
  - Timeline::invalidate_index()
    ↓
GUI State: Timeline UI Updated
    ↓
Result: Clips Moved to New Position/Layer
```

**Data Transformations**:
- GUI: Drag coordinates → ClipMoveCtx
- Domain Logic: ClipMoveCtx → ClipMoveHistoryCtx → Updated Clip
- Network: Updated Clip → rkyv bytes
- GUI: rkyv bytes → Updated Clip → UI representation

**Data Generated**:
- ClipMoveHistoryCtx (for undo/redo)
- ChangeSet entries

**Data Modified**:
- Timeline.clips (multiple)
- Layer.clips (multiple layers)
- Timeline.changes
- Timeline.chunk_index

---

### 4. ビデオストリーム初期化フロー

```
User Action (InitStream)
    ↓
GUI: File Selection Dialog
    ↓
GUI State: Selected File Path
    ↓
GUI Processing:
  - Validate file path
  - Check file accessibility
    ↓
C++ FFI: Stream Request Creation
    ↓
Rust FFI: Request::InitStream { path } Serialization
    ↓
Network: TCP Send
    ↓
Core: Request Handler
    ↓
Domain Logic:
  - VideoStreamer::new(path)
    - FFmpeg: avformat_open_input()
    - FFmpeg: avformat_find_stream_info()
    - FFmpeg: avcodec_find_decoder()
    - FFmpeg: avcodec_alloc_context3()
    - FFmpeg: avcodec_open2()
  - state.get_or_create_resource_id(path)
    - Check path_to_stream mapping
    - Generate new ResourceId if not exists
  - VideoStreamer::get_init_packet(path, resource_id)
    - Extract codec_id, width, height, time_base, extradata
    ↓
State Mutation:
  - ServerState.streams: {resource_id: VideoStreamer}
  - ServerState.path_to_stream: {path: StreamState::Loaded(resource_id)}
    ↓
Response: Response::StreamMetadata { path, resource_id, codec_id, width, height, time_base, extradata }
    ↓
Network: TCP Send
    ↓
GUI: Response Handler
    ↓
C++: StreamPlayer Creation
  - Store ResourceId
  - Store codec information
  - Initialize decode buffer
    ↓
GUI State: Media Library Updated
    ↓
Result: Video Stream Ready for Playback
```

**Data Transformations**:
- GUI: File path → InitStream request
- Domain Logic: File path → VideoStreamer → StreamMetadata
- Network: StreamMetadata → rkyv bytes
- GUI: rkyv bytes → StreamPlayer

**Data Generated**:
- ResourceId (new or reused)
- VideoStreamer
- StreamPlayer
- StreamMetadata

**Data Modified**:
- ServerState.streams
- ServerState.path_to_stream

---

### 5. 範囲クリップ取得フロー

```
User Action (FetchClipsInRange)
    ↓
GUI: Timeline Scroll/Zoom
    ↓
GUI State: Current View Range
    ↓
GUI Processing:
  - Calculate visible range (start, end)
  - Detect range change
    ↓
C++ FFI: Fetch Request Creation
    ↓
Rust FFI: Request::FetchClipsInRange { timeline_id, range } Serialization
    ↓
Network: TCP Send
    ↓
Core: Request Handler
    ↓
Domain Logic:
  - state.network.update_client_view(client_id, timeline_id, range)
    - Update client view tracking
  - Timeline::query_range(range)
    - Check ChunkIndex
    - If None: Build ChunkIndex
      - Collect all (layer_id, position, clip_id) entries
      - ChunkIndex::build(entries)
    - ChunkIndex.candidates(range)
    - Filter by actual overlap: clip.position < range.end && clip.position + clip.duration > range.start
    - Collect matching (layer_id, clip) pairs
    ↓
State Mutation:
  - ServerState.network.client_views: {client_id: {timeline_id: range}}
  - Timeline.chunk_index: ChunkIndex (if built)
    ↓
Response: Response::UpdateClip { timeline_id, clips: Vec<(layer_id, clip)> }
    ↓
Network: TCP Send
    ↓
GUI: Response Handler
    ↓
C++: Timeline::merge_fetched_clips(entries)
  - For each (layer_id, clip):
    - Layer.clips.insert(clip.position, clip.id)
    - Timeline.clips.insert(clip.id, clip)
  - Timeline::invalidate_index()
    ↓
GUI State: Timeline UI Updated
    ↓
Result: Visible Clips Displayed
```

**Data Transformations**:
- GUI: View coordinates → Range<TimelineTick>
- Domain Logic: Range → Vec<Clip> (via ChunkIndex)
- Network: Vec<Clip> → rkyv bytes
- GUI: rkyv bytes → Vec<Clip> → UI representation

**Data Generated**:
- ChunkIndex (if not exists)
- Client view record

**Data Modified**:
- ServerState.network.client_views
- Timeline.chunk_index (client side)
- Timeline.clips (client side)
- Layer.clips (client side)

---

### 6. レイヤー/フォルダー追加フロー

```
User Action (AddLayer/AddFolder)
    ↓
GUI: Layer/Folder Addition Menu
    ↓
GUI State: Parent Selection, Name Input
    ↓
GUI Processing:
  - Get parent_folder_id (None for root)
  - Get insert_index (None for end)
  - Get name
    ↓
C++ FFI: Command Request Creation
    ↓
Rust FFI: Request::Command { CommandRequest::AddLayer/AddFolder } Serialization
    ↓
Network: TCP Send
    ↓
Core: Request Handler
    ↓
Domain Logic:
  - command_to_history() → CommandHistory::AddLayer/AddFolder
  - handle_command_action()
  - Project::insert_layer_in_timeline() or Project::insert_folder_in_timeline()
  - Timeline::insert_layer() or Timeline::insert_folder()
    - IdGenerator::next_layer_id() or next_folder_id()
    - Layer::new() or LayerFolder creation
    - Timeline.layers.insert() or Timeline.outline.folders.insert()
    - Timeline.outline.insert_layer() or insert_folder()
    - Timeline::touch_upsert() or touch_outline_folder_upserted()
    - Timeline::touch_outline_children_changed()
    ↓
State Mutation:
  - Timeline.layers: {layer_id: Layer} (AddLayer only)
  - Timeline.outline.folders: {folder_id: LayerFolder} (AddFolder only)
  - Timeline.outline.roots or folder.children: OutlineNode added
  - Timeline.changes: layers_upserted or outline_folders_upserted + outline_children_changed
    ↓
Dirty Notification: notify_dirty()
    ↓
Change Sync:
  - Project::drain_changes() → ChangeSet
  - Response::UpdateLayer or UpdateOutline
    ↓
Network: TCP Send
    ↓
GUI: Response Handler
    ↓
C++: Timeline::apply_layer_meta() or apply_outline_folder_meta() + apply_outline_children()
    ↓
GUI State: Layer/Folder UI Updated
    ↓
Result: Layer/Folder Added to Timeline
```

**Data Transformations**:
- GUI: User input → Layer/Folder parameters
- Domain Logic: Parameters → Layer/Folder entity
- Network: Layer/Folder → rkyv bytes
- GUI: rkyv bytes → Layer/Folder entity → UI representation

**Data Generated**:
- LayerId or LayerFolderId (new)
- Layer or LayerFolder entity
- ChangeSet entries

**Data Modified**:
- Timeline.layers (AddLayer only)
- Timeline.outline.folders (AddFolder only)
- Timeline.outline structure
- Timeline.changes

---

## データのライフサイクル

### Clipのライフサイクル
```
File (Disk)
  ↓ Import
InitStream → VideoStreamer → ResourceId
  ↓ AddClip
Clip Entity Created (Timeline.clips, Layer.clips)
  ↓ Move/Modify
Clip Entity Updated
  ↓ Remove
Clip Entity Deleted
  ↓
VideoStreamer Removed (when no references)
```

### Timelineのライフサイクル
```
NewProject
  ↓
Timeline Created (with default layers)
  ↓ Layer/Folder Operations
Timeline Structure Modified
  ↓ Clip Operations
Timeline Content Modified
  ↓
Timeline Clone (for Independent)
  ↓
Timeline Deleted (when no references)
```

### ChangeSetのライフサイクル
```
Timeline Operation
  ↓
ChangeSet Updated (mark_*)
  ↓ Dirty Notification
ChangeSet Drained (drain_changes)
  ↓ Response Generation
ChangeSet Consumed
  ↓ Next Operation
New ChangeSet Created
```

### ChunkIndexのライフサイクル
```
Timeline Creation
  ↓
ChunkIndex: None
  ↓ First query_range
ChunkIndex Built (from all clips)
  ↓ Clip Operation
ChunkIndex Invalidated (None)
  ↓ Next query_range
ChunkIndex Rebuilt
```

## データの移動パターン

### 1. ユーザー入力 → ドメインエンティティ
- GUIがユーザー入力をパラメータに変換
- ネットワーク経由でCoreに転送
- Coreがパラメータからドメインエンティティを生成

### 2. ドメインエンティティ → GUI表示
- Coreがドメインエンティティをシリアライズ
- ネットワーク経由でGUIに転送
- GUIがデシリアライズしてUI表示に変換

### 3. 増分更新
- CoreがChangeSetを生成
- 変更があったエンティティのみを転送
- GUIが差分を適用

### 4. キャッシュ戦略
- ChunkIndex: 遅延構築、無効化時に再構築
- Client View: サーバー側で記録、増分更新の最適化に使用
- VideoStreamer: サーバー側でキャッシュ、ResourceIdで再利用

## Open Questions

1. **Media Import**: InitStreamとAddClipのタイミング関係は？
2. **Clip Duration**: durationの自動計算ロジックは？
3. **Undo/Redo**: CommandHistoryの実際の活用方法は？
4. **Resource Cleanup**: VideoStreamerの解放タイミングと条件は？
5. **Cache Invalidation**: ChunkIndexの無効化条件の網羅性は？
6. **Error Recovery**: データ転送失敗時のリカバリーは？
7. **Consistency**: GUIとCoreの状態不一致の検出と修正は？
