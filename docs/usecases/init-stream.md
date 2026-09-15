# UC-5: InitStream

## Feature
ビデオファイルからストリームを初期化する

## User Action
ユーザーがビデオファイルをプロジェクトにインポート

## Preconditions
- プロジェクトが開かれている
- ビデオファイルがアクセス可能である

## Main Flow

### User → GUI
1. ユーザーがファイル選択ダイアログを開く
2. ユーザーがビデオファイルを選択
3. ユーザーがインポートを確定

### GUI → Request
4. GUIがファイルパスを取得
5. GUIがC++ FFIを呼び出し
6. Rust FFIが `Request::InitStream { path: String }` をシリアライズ
7. `ClientNetworkHandler::send()` でTCP送信

### Request → Core
8. CoreサーバーがTCP受信
9. `ServerNetworkHandler::parse_and_handle_request()` でデシリアライズ
10. `on_request_receive(ArchivedRequest::InitStream)` を呼び出し

### Core → Domain Logic
11. `VideoStreamer::new(path)` を実行:
    - FFmpegでビデオファイルをオープン
    - ストリーム情報を取得
    - デコーダーを初期化
12. `state.get_or_create_resource_id(path)` でResourceIdを取得/生成
13. `VideoStreamer::get_init_packet(path, resource_id)` を実行:
    - `StreamMetadata` を生成:
      - `path`: ファイルパス
      - `resource_id`: リソースID
      - `codec_id`: コーデックID
      - `width`: 映像幅
      - `height`: 映像高
      - `time_base`: タイムベース
      - `extradata`: エクストラデータ（コーデック情報）
14. `state.streams.insert(resource_id, streamer)` でVideoStreamerを保存
15. `state.path_to_stream.insert(path, StreamState::Loaded(resource_id))` でパス→ResourceIdマッピングを保存

### Domain State Mutation
- **VideoStreamer Created**: 新しいVideoStreamerインスタンス
- **ResourceId Generated/Retrieved**: パスに対応するResourceId
- **ServerState.streams Updated**: resource_id → VideoStreamerマッピング
- **ServerState.path_to_stream Updated**: path → StreamStateマッピング

### Response → GUI
16. `Response::StreamMetadata { path, resource_id, codec_id, width, height, time_base, extradata }` を生成
17. `ServerNetworkHandler::send(client_id, &res)` でTCP送信

### GUI → GUI Update
18. GUIがTCP受信
19. `ClientNetworkHandler::parse_and_handle_responce()` でデシリアライズ
20. `on_responce_recveve(Response::StreamMetadata)` を呼び出し
21. FFIコールバックでC++側に通知
22. C++がStreamPlayerを作成:
    - ResourceIdを保存
    - コーデック情報を保存
    - デコードバッファを初期化
23. GUIがメディアライブラリにファイルを表示

## Result
- ビデオストリームが初期化される
- ResourceIdが割り当てられる
- GUIにメディア情報が表示される

## Side Effects
- ServerStateにストリーム情報が保存される
- 同じパスの再リクエストで既存のResourceIdが再利用される

## Dependencies
- FFmpegライブラリ
- ビデオファイルのアクセス権限
- サポートされているコーデック

## Related Features
- FetchStreamData
- Clip追加（Videoクリップ）
- レンダリング

## Alternative Flow

### ファイルオープン失敗
- `VideoStreamer::new(path)` でFFmpegオープン失敗
- `EsotereelError::IoError` を返す
- **Unknown**: GUI側でのエラー表示

### 既存のストリーム
- 同じパスが既に `path_to_stream` に存在
- 既存のResourceIdを再利用
- 新しいVideoStreamerは作成されない

### 不正なファイル形式
- FFmpegがファイルをデコードできない
- **Unknown**: エラー処理

## Open Questions
- 同じファイルを複数回インポートした場合の挙動は？
- ストリームの解放タイミングは？
- リソースリーク対策はあるか？
- オーディオストリームの初期化は？

## Error Handling
- **IoError**: ファイルオープン失敗
- **InvalidFormat**: 不正なファイル形式
- **Network Error**: 通信失敗時の処理は不明
