# UC-2: AddClip

## Feature
タイムラインにクリップを追加する

## User Action
ユーザーがメディアファイルを選択し、タイムライン上の位置にドロップ

## Preconditions
- プロジェクトが開かれている
- ターゲットレイヤーが存在している
- ユーザーが追加位置を指定している

## Main Flow

### User → GUI
1. ユーザーがメディアファイルを選択
2. ユーザーがタイムライン上の位置とレイヤーを指定
3. ユーザーがドロップ操作を実行

### GUI → Request
4. GUIがClipパラメータを構築:
   - `layer_id`: ターゲットレイヤーID
   - `position`: ドロップ位置（TimelineTick）
   - `duration`: メディア長さ（TimelineTick）
   - `kind_id`: NamespacedID（プラグイン定義）
   - `properties`: プロパティマップ
   - `translates`: 変換パラメータ
5. GUIがC++ FFIを呼び出し
6. Rust FFIが `Request::Command { command: CommandRequest::AddClip { ... }, timeline_id }` をシリアライズ
7. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
8. CoreサーバーがTCP受信
9. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
10. `on_request_receive(ArchivedRequest::Command)` を呼び出し

### Core → Domain Logic
11. `command_to_history()` でCommandRequestをCommandHistoryに変換
12. `handle_command_action()` を呼び出し
13. `clip_add_core()` を実行:
    - `Project::timeline_mut(timeline_id)` でTimeline取得
    - `Timeline::new_clip_in()` を呼び出し:
      - 重複チェック: `can_place_clip_at()` で衝突検出
      - `IdGenerator::next_clip_id()` で新規ClipId生成
      - `Clip::new()` でClip作成
      - `Layer.clips.insert(position, clip_id)` でレイヤーに追加
      - `Timeline.clips.insert(clip_id, clip)` で実体を追加
      - `Timeline::touch_upsert(clip_id)` でChangeSet更新
      - `Timeline::invalidate_index()` でChunkIndex無効化

### Domain State Mutation
- **Clip Created**: 新しいClipインスタンス
- **Layer Updated**: Layer.clipsに新しいposition→clip_idマッピング
- **Timeline.clips Updated**: Clip実体の追加
- **ChangeSet Updated**: `clips_upserted` にclip_id追加
- **ChunkIndex Invalidated**: 範囲検索キャッシュ無効化

### Response → GUI
14. `handle_command_action()` 完了後、`state.network.notify_dirty()` を呼び出し
15. Dirty通知により、変更同期タスクが起動
16. `Project::drain_changes()` でChangeSetを取得
17. ChangeSetからResponse生成:
    - `Response::UpdateClip { timeline_id, clips: [(layer_id, clip)] }`
18. `ServerNetworkHandler::send()` でTCP送信

### GUI → GUI Update
19. GUIがTCP受信
20. `ClientNetworkHandler::parse_and_handle_responce()` でデシリアライズ
21. `on_responce_recveve(Response::UpdateClip)` を呼び出し
22. FFIコールバックでC++側に通知
23. C++が `Timeline::upsert_clip_from_network()` を呼び出し:
    - 全レイヤーから古いclip_id参照を削除
    - ターゲットレイヤーに新しい参照を追加
    - Timeline.clipsにClip実体を更新
    - ChunkIndex無効化
24. GUIがタイムラインUIを更新

## Result
- クリップがタイムラインに追加される
- クリップが指定位置に配置される
- GUIにクリップが表示される

## Side Effects
- ChangeSetが更新される
- ChunkIndexが無効化される
- 他のクライアント（いる場合）にも変更が通知される

## Dependencies
- プロジェクト存在
- ターゲットレイヤー存在
- プラグインのClipKind定義
- IdGenerator（ClipId生成）

## Related Features
- ClipsMove
- RemoveClip
- FetchClipsInRange
- ChangeSet管理

## Alternative Flow

### 重複検出失敗
- `Timeline::new_clip_in()` で重複検出
- `EsotereelError::ClipOverlap` を返す
- **Unknown**: GUI側でのエラー表示

### レイヤー不存在
- `Project::timeline_mut()` でTimeline取得失敗
- `EsotereelError::TimelineNotFound` を返す
- **Unknown**: GUI側でのエラー表示

### 無効なkind_id
- プラグインにClipKindが存在しない
- **Unknown**: エラー処理

## Open Questions
- メディアファイルのパスはどのように管理されるか？
- Videoクリップの場合、InitStreamはいつ呼ばれるか？
- durationの自動計算はあるか？
- プロパティのデフォルト値はどうなるか？

## Error Handling
- **ClipOverlap**: 位置が重複している場合
- **TimelineNotFound**: タイムラインが存在しない場合
- **LayerNotFound**: レイヤーが存在しない場合
- **Network Error**: 通信失敗時の処理は不明
