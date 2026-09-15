# UC-1: NewProject

## Feature
新しいプロジェクトを作成する

## User Action
ユーザーが「新規プロジェクト」を選択

## Preconditions
- Coreサーバーが起動している
- GUIがCoreサーバーに接続している

## Main Flow

### User → GUI
1. ユーザーが「新規プロジェクト」メニュー/ボタンをクリック

### GUI → Request
2. GUIがC++ FFIを呼び出し
3. Rust FFIが `Request::NewProject` をシリアライズ
4. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
5. CoreサーバーがTCP受信
6. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
7. `on_request_receive(ArchivedRequest::NewProject)` を呼び出し

### Core → Domain Logic
8. `Project::new()` で空のProjectを作成
9. `Project::insert_timeline(60.0)` でデフォルトタイムラインを作成
   - Timeline::new()が4つのデフォルトレイヤーを作成
   - Timeline::new()がデフォルトOutlineを作成
10. `Project::timelines_meta()` でメタデータを生成

### Domain State Mutation
- **Project Created**: 新しいProjectインスタンス
- **Timeline Created**: id=0, fps=60.0, 4 Layer, default Outline
- **Layers Created**: Layer 1-4 (id=1-4)
- **ServerState Updated**: `state.project = Some(Arc::new(RwLock::new(new_project)))`
- **Client View Updated**: `client_id` のビューを全タイムライン範囲に設定

### Response → GUI
11. `Response::ProjectMeta { timelines }` を生成
12. `ServerNetworkHandler::send(client_id, &cmd)` でTCP送信

### GUI → GUI Update
13. GUIがTCP受信
14. `ClientNetworkHandler::parse_and_handle_responce()` でデシリアライズ
15. `on_responce_recveve(Response::ProjectMeta)` を呼び出し
16. FFIコールバックでC++側に通知
17. C++が `Timeline::from_meta()` でTimeline骨格を作成（Clipなし）
18. GUIがタイムラインUIを表示

## Result
- 新しいプロジェクトが作成される
- デフォルトタイムライン（id=0, fps=60.0）が作成される
- 4つのデフォルトレイヤーが作成される
- GUIにタイムラインが表示される

## Side Effects
- ServerState.projectが上書きされる（以前のプロジェクトは破棄）
- クライアントの表示範囲がリセットされる
- ネットワーク接続が確認される

## Dependencies
- Coreサーバー起動
- ネットワーク接続
- IdGenerator（TimelineId、LayerId生成）

## Related Features
- ProjectAll
- Timeline操作
- Layer操作

## Open Questions
- 以前のプロジェクトの保存確認はあるか？
- プロジェクト設定（fps等）のユーザー指定は可能か？
- 複数プロジェクトの同時オープンは可能か？

## Error Handling
- **Network Error**: 接続失敗時のGUI処理は不明
- **State Error**: Project作成失敗時の処理は不明
