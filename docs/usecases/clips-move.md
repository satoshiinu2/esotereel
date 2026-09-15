# UC-3: ClipsMove

## Feature
複数のクリップを移動する（位置、持続時間、レイヤー間）

## User Action
ユーザーがクリップを選択し、新しい位置/レイヤーにドラッグ

## Preconditions
- プロジェクトが開かれている
- 移動対象のクリップが存在している
- 移動先レイヤーが存在している

## Main Flow

### User → GUI
1. ユーザーが移動対象のクリップを選択
2. ユーザーがドラッグ操作を開始
3. ユーザーが新しい位置/レイヤーでドロップ

### GUI → Request
4. GUIが移動パラメータを構築:
   - `clips: Vec<ClipMoveCtx>`:
     - `clip_id`: 移動対象クリップID
     - `new_position`: 新しい位置
     - `new_duration`: 新しい持続時間
     - `new_layer_id`: 新しいレイヤーID
5. GUIがC++ FFIを呼び出し
6. Rust FFIが `Request::Command { command: CommandRequest::ClipsMove { clips }, timeline_id }` をシリアライズ
7. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
8. CoreサーバーがTCP受信
9. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
10. `on_request_receive(ArchivedRequest::Command)` を呼び出し

### Core → Domain Logic
11. `command_to_history()` でCommandRequestをCommandHistoryに変換:
    - 各ClipMoveCtxについて現在の状態を取得
    - `Timeline::get_clip_and_layer(clip_id)` で現在のposition, duration, layer_idを取得
    - `ClipMoveHistoryCtx` を構築（old_*, new_* を保持）
12. `handle_command_action()` を呼び出し
13. `clip_move_mul_core()` を実行:
    - 各ClipMoveHistoryCtxについて:
      - `Timeline::get_clip_and_layer()` で現在のレイヤーを取得
      - `Timeline::remove_clip_by_id_in(old_layer_id, clip_id)` で旧位置から削除
      - `Timeline::place_clip(new_layer_id, updated_clip)` で新位置に配置:
        - 旧レイヤーから参照を削除
        - Clipのposition, durationを更新
        - 新レイヤーに参照を追加
        - `Timeline::touch_upsert(clip_id)` でChangeSet更新

### Domain State Mutation
- **Clips Updated**: 複数のClipのposition, duration, layer_idが変更
- **Layers Updated**: 複数のLayer.clipsが更新（旧レイヤーから削除、新レイヤーに追加）
- **Timeline.clips Updated**: Clip実体の更新
- **ChangeSet Updated**: 各clip_idが `clips_upserted` に追加
- **ChunkIndex Invalidated**: 範囲検索キャッシュ無効化

### Response → GUI
14. `handle_command_action()` 完了後、`state.network.notify_dirty()` を呼び出し
15. Dirty通知により、変更同期タスクが起動
16. `Project::drain_changes()` でChangeSetを取得
17. ChangeSetからResponse生成:
    - `Response::UpdateClip { timeline_id, clips: [(new_layer_id, updated_clip), ...] }`
18. `ServerNetworkHandler::send()` でTCP送信

### GUI → GUI Update
19. GUIがTCP受信
20. `ClientNetworkHandler::parse_and_handle_responce()` でデシリアライズ
21. `on_responce_recveve(Response::UpdateClip)` を呼び出し
22. FFIコールバックでC++側に通知
23. C++が `Timeline::upsert_clip_from_network()` を各クリップについて呼び出し:
    - 全レイヤーから古いclip_id参照を削除
    - 新しいレイヤーに新しい参照を追加
    - Timeline.clipsにClip実体を更新
    - ChunkIndex無効化
24. GUIがタイムラインUIを更新

## Result
- クリップが新しい位置に移動される
- クリップが新しいレイヤーに移動される
- クリップの持続時間が変更される
- GUIに更新が反映される

## Side Effects
- ChangeSetが更新される
- ChunkIndexが無効化される
- 他のクライアント（いる場合）にも変更が通知される
- アンドゥ/リドゥ用の履歴が記録される

## Dependencies
- プロジェクト存在
- 移動対象クリップ存在
- 移動先レイヤー存在
- IdGenerator（既存ClipId使用）

## Related Features
- AddClip
- RemoveClip
- Layer操作
- CommandHistory

## Alternative Flow

### 重複検出失敗
- `Timeline::place_clip()` 内で重複チェック
- `EsotereelError::ClipOverlap` を返す
- **Unknown**: GUI側でのエラー表示とロールバック

### クリップ不存在
- `Timeline::get_clip_and_layer()` でクリップ取得失敗
- `EsotereelError::ClipNotFound` を返す
- **Unknown**: GUI側でのエラー表示

### レイヤー不存在
- 移動先レイヤーが存在しない
- `EsotereelError::LayerNotFound` を返す
- **Unknown**: GUI側でのエラー表示

## Open Questions
- 複数クリップの移動で一部が失敗した場合、他の移動はロールバックされるか？
- ドラッグ中のリアルタイムプレビューはどうなるか？
- 移動中の他のクライアントの編集との競合はどう処理されるか？

## Error Handling
- **ClipNotFound**: 移動対象クリップが存在しない場合
- **LayerNotFound**: 移動先レイヤーが存在しない場合
- **ClipOverlap**: 移動先で重複が発生する場合
- **Network Error**: 通信失敗時の処理は不明
