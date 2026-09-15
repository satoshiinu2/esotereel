# UC-4: AddLayer / AddFolder

## Feature
レイヤーまたはフォルダーを追加する

## User Action
ユーザーが「レイヤー追加」または「フォルダー追加」を選択

## Preconditions
- プロジェクトが開かれている
- ターゲットタイムラインが存在している

## Main Flow

### User → GUI
1. ユーザーが「レイヤー追加」または「フォルダー追加」を選択
2. ユーザーが名前を入力（省略可能）
3. ユーザーが親フォルダーと挿入位置を指定（省略可能）

### GUI → Request
4. GUIがパラメータを構築:
   - `parent_folder_id`: 親フォルダーID（Noneでルート）
   - `insert_index`: 挿入位置（Noneで末尾）
   - `name`: レイヤー/フォルダー名
5. GUIがC++ FFIを呼び出し
6. Rust FFIが `Request::Command { command: CommandRequest::AddLayer { ... } or AddFolder { ... }, timeline_id }` をシリアライズ
7. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
8. CoreサーバーがTCP受信
9. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
10. `on_request_receive(ArchivedRequest::Command)` を呼び出し

### Core → Domain Logic
11. `command_to_history()` でCommandRequestをCommandHistoryに変換
12. `handle_command_action()` を呼び出し

#### AddLayerの場合
13. `Project::insert_layer_in_timeline()` を実行:
    - `Project::timeline_mut(timeline_id)` でTimeline取得
    - `Timeline::insert_layer()` を呼び出し:
      - `IdGenerator::next_layer_id()` で新規LayerId生成
      - `Layer::new(id, name)` でLayer作成
      - `Timeline.layers.insert(id, layer)` で追加
      - `Timeline::touch_upsert(id)` でChangeSet更新
      - `Timeline.outline.insert_layer(id, parent, index)` でOutline更新
      - `Timeline::touch_outline_children_changed(parent)` でChangeSet更新

#### AddFolderの場合
13. `Project::insert_folder_in_timeline()` を実行:
    - `Project::timeline_mut(timeline_id)` でTimeline取得
    - `Timeline::insert_folder()` を呼び出し:
      - `IdGenerator::next_folder_id()` で新規LayerFolderId生成
      - `Timeline.outline.insert_folder(id, name, parent, index)` でFolder作成
      - `Timeline::touch_outline_folder_upserted(id)` でChangeSet更新
      - `Timeline::touch_outline_children_changed(parent)` でChangeSet更新

### Domain State Mutation

#### AddLayer
- **Layer Created**: 新しいLayerインスタンス
- **Timeline.layers Updated**: Layerの追加
- **Outline Updated**: Layerノードの追加
- **ChangeSet Updated**: `layers_upserted` にlayer_id追加、`outline_children_changed` 更新

#### AddFolder
- **LayerFolder Created**: 新しいLayerFolderインスタンス
- **Outline Updated**: Folderノードの追加
- **ChangeSet Updated**: `outline_folders_upserted` にfolder_id追加、`outline_children_changed` 更新

### Response → GUI
14. `handle_command_action()` 完了後、`state.network.notify_dirty()` を呼び出し
15. Dirty通知により、変更同期タスクが起動
16. `Project::drain_changes()` でChangeSetを取得
17. ChangeSetからResponse生成:

#### AddLayer
- `Response::UpdateLayer { timeline_id, layers: [LayerMeta] }`
- `Response::UpdateOutline { timeline_id, folders: [], children: [(parent, [OutlineNode])] }`

#### AddFolder
- `Response::UpdateOutline { timeline_id, folders: [(folder_id, Meta)], children: [(parent, [OutlineNode])] }`

18. `ServerNetworkHandler::send()` でTCP送信

### GUI → GUI Update
19. GUIがTCP受信
20. `ClientNetworkHandler::parse_and_handle_responce()` でデシリアライズ
21. `on_responce_recveve(Response::UpdateLayer or UpdateOutline)` を呼び出し
22. FFIコールバックでC++側に通知
23. C++が対応する更新メソッドを呼び出し:
    - UpdateLayer: `Timeline::apply_layer_meta()`
    - UpdateOutline: `Timeline::apply_outline_folder_meta()`, `Timeline::apply_outline_children()`
24. GUIがレイヤー/フォルダーUIを更新

## Result
- レイヤーまたはフォルダーがタイムラインに追加される
- GUIに新しいレイヤー/フォルダーが表示される

## Side Effects
- ChangeSetが更新される
- Outline構造が変更される
- 他のクライアント（いる場合）にも変更が通知される

## Dependencies
- プロジェクト存在
- ターゲットタイムライン存在
- 親フォルダー存在（指定場合）
- IdGenerator（LayerId/LayerFolderId生成）

## Related Features
- RemoveLayer/RemoveFolder
- Layer操作
- Outline管理

## Alternative Flow

### タイムライン不存在
- `Project::timeline_mut()` でTimeline取得失敗
- `EsotereelError::TimelineNotFound` を返す
- **Unknown**: GUI側でのエラー表示

### 親フォルダー不存在
- 指定された親フォルダーが存在しない
- **Unknown**: エラー処理

### 循環参照（Folder移動時）
- Folderを自分自身の子孫に移動しようとする
- `EsotereelError::InvalidLayerMove` を返す
- **Unknown**: GUI側でのエラー表示

## Open Questions
- レイヤー/フォルダーのデフォルト名はどうなるか？
- 挿入位置が範囲外の場合の処理は？
- フォルダーのブレンドモードや不透明度の初期値は？

## Error Handling
- **TimelineNotFound**: タイムラインが存在しない場合
- **InvalidLayerMove**: 循環参照になる場合
- **Network Error**: 通信失敗時の処理は不明
