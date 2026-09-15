# UC-6: FetchClipsInRange

## Feature
指定した時間範囲内のクリップを取得する

## User Action
ユーザーがタイムラインをスクロールまたはズーム

## Preconditions
- プロジェクトが開かれている
- ターゲットタイムラインが存在している

## Main Flow

### User → GUI
1. ユーザーがタイムラインをスクロール
2. GUIが現在の表示範囲を計算
3. GUIが表示範囲が変更されたことを検知

### GUI → Request
4. GUIがパラメータを構築:
   - `timeline_id`: ターゲットタイムラインID
   - `range`: 表示範囲 (Range<TimelineTick>)
5. GUIがC++ FFIを呼び出し
6. Rust FFIが `Request::FetchClipsInRange { timeline_id, range }` をシリアライズ
7. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
8. CoreサーバーがTCP受信
9. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
10. `on_request_receive(ArchivedRequest::FetchClipsInRange)` を呼び出し

### Core → Domain Logic
11. `state.network.update_client_view(client_id, timeline_id, range)` でクライアントの表示範囲を記録
12. `Project::timeline(timeline_id)` でTimeline取得
13. `Timeline::query_range(range)` を実行:
    - ChunkIndexが存在しない場合、遅延構築:
      - 全Layerの全Clipからエントリを収集
      - `ChunkIndex::build(entries)` でインデックス構築
    - ChunkIndexから候補Clipを取得
    - 各候補について範囲オーバーラップチェック:
      - `clip.position < range.end && clip.position + clip.duration > range.start`
    - オーバーラップしているClipを収集
14. 収集したClipを `(layer_id, clip.clone())` のリストに変換

### Domain State Mutation
- **Client View Updated**: クライアントの表示範囲が記録される
- **ChunkIndex Built**: （必要な場合）インデックスが構築される
- **No Direct State Changes**: 読み取り操作のみ

### Response → GUI
15. `Response::UpdateClip { timeline_id, clips: Vec<(layer_id, clip)> }` を生成
16. `ServerNetworkHandler::send(client_id, &cmd)` でTCP送信

### GUI → GUI Update
17. GUIがTCP受信
18. `ClientNetworkHandler::parse_and_handle_responce()` でデシリアライズ
19. `on_responce_recveve(Response::UpdateClip)` を呼び出し
20. FFIコールバックでC++側に通知
21. C++が `Timeline::merge_fetched_clips(entries)` を呼び出し:
    - 各 (layer_id, clip) について:
      - Layer.clips.insert(clip.position, clip.id) で参照を追加
      - Timeline.clips.insert(clip.id, clip) で実体を追加
    - `Timeline::invalidate_index()` でChunkIndex無効化
22. GUIがタイムラインUIを更新

## Result
- 指定範囲内のクリップがGUIに表示される
- クライアントの表示範囲がサーバーに記録される

## Side Effects
- ChunkIndexが（必要な場合）構築される
- クライアント側のChunkIndexが無効化される
- サーバー側のクライアント表示範囲が更新される

## Dependencies
- プロジェクト存在
- ターゲットタイムライン存在
- ChunkIndex（遅延構築）

## Related Features
- AddClip
- ClipsMove
- RemoveClip
- ChangeSet管理

## Alternative Flow

### タイムライン不存在
- `Project::timeline()` でTimeline取得失敗
- `EsotereelError::TimelineNotFound` を返す
- **Unknown**: GUI側でのエラー表示

### 範囲内にクリップなし
- `Timeline::query_range()` が空のVecを返す
- `Response::UpdateClip { timeline_id, clips: [] }` を送信
- GUIはクリップを表示しない

## Open Questions
- スクロール中の頻繁なリクエストに対する最適化は？
- クライアント側のキャッシュ戦略は？
- 大量のクリップがある場合のパフォーマンスは？

## Error Handling
- **TimelineNotFound**: タイムラインが存在しない場合
- **Network Error**: 通信失敗時の処理は不明

## 注記
- この機能は主にGUIの表示範囲同期に使用される
- ChunkIndexは遅延構築されるため、初回呼び出し時にコストがかかる
- クライアントの表示範囲はサーバー側で記録され、増分更新の最適化に使用される
