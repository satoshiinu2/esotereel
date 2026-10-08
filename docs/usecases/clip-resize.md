# UC-4: ClipsResize

## Feature
- **ClipsResize**: 複数のクリップのサイズを変更する（位置、持続時間、source_offset）

## User Action
- **ClipsResize**: ユーザーがクリップの端をドラッグしてサイズを変更

## Preconditions
- プロジェクトが開かれている
- 対象のクリップが存在している

## Main Flow

### User → GUI
1. ユーザーがリサイズ対象のクリップを選択
2. ユーザーがクリップの端をドラッグ操作を開始
3. ユーザーがドラッグを終了

### GUI → Request
4. GUIがパラメータを構築:
   - **ClipsResize**: `clips: Vec<ClipResizeCtx>`:
     - `clip_id`: リサイズ対象クリップID
     - `left_edge`: 左端をドラッグ中かどうか
     - `frame_delta`: 変更フレーム数
5. GUIがC++ FFIを呼び出し
6. Rust FFIが `Request::Command { command: CommandRequest::ClipsResize { clips }, timeline_id }` をシリアライズ
7. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
8. CoreサーバーがTCP受信
9. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
10. `on_request_receive(ArchivedRequest::Command)` を呼び出し

### Core → Domain Logic
11. `command_to_history()` でCommandRequestをCommandHistoryに変換:
    - **ClipsResize**: 各ClipResizeCtxについて現在の状態を取得
      - `Timeline::get_clip(clip_id)` で現在のposition, duration, source_offsetを取得
      - `ClipResizeHistoryCtx` を構築（old_*, new_*, left_edge, source_offset を保持）
12. `handle_command_action()` を呼び出し
13. コア関数を実行:
    - **ClipsResize**: `clip_resize_core()` を実行:
      - 各ClipResizeHistoryCtxについて:
        - 重なり判定: `Timeline::can_place_clip_at()` でリサイズ後の重なりをチェック
        - 重なっている場合は何もしない
        - Clipのposition, durationを直接更新
        - source_offsetを更新（必要な場合）

### Domain State Mutation
- **ClipsResize**:
  - 複数のClipのposition, duration, source_offsetが変更
  - Layer.clipsは変更なし（同一レイヤー内）
- **共通**:
  - Timeline.clips Updated: Clip実体の更新
  - ChangeSet Updated: 各clip_idが `clips_upserted` に追加
  - ChunkIndex Invalidated: 範囲検索キャッシュ無効化

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
- **ClipsResize**:
  - クリップのサイズが変更される
  - source_offsetが更新される（左端リサイズ時）
- GUIに更新が反映される

## Side Effects
- ChangeSetが更新される
- ChunkIndexが無効化される
- 他のクライアント（いる場合）にも変更が通知される
- アンドゥ/リドゥ用の履歴が記録される

## Dependencies
- プロジェクト存在
- リサイズ対象クリップ存在
- IdGenerator（既存ClipId使用）

## Related Features
- AddClip
- RemoveClip
- CommandHistory

## Alternative Flow

### 重複検出失敗
- `Timeline::place_clip()` 内で重複チェック
- `EsotereelError::ClipOverlap` を返す
- **Unknown**: GUI側でのエラー表示とロールバック

### クリップ不存在
- `Timeline::get_clip()` でクリップ取得失敗
- `EsotereelError::ClipNotFound` を返す
- **Unknown**: GUI側でのエラー表示

## Open Questions
- ドラッグ中のリアルタイムプレビューはどうなるか？
- リサイズ中の他のクライアントの編集との競合はどう処理されるか？

## Error Handling
- **ClipNotFound**: リサイズ対象クリップが存在しない場合
- **ClipOverlap**: リサイズ先で重複が発生する場合
- **Network Error**: 通信失敗時の処理は不明
