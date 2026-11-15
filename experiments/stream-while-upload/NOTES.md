# アップロード中ファイルの先行デコード — 技術検証 NOTES

検証日: 2026-11-15
場所: `~/workspace/stream-while-upload-proto/`（Esotereel 本体とは独立した試作。git init のみ）

## 仮説

FFmpeg の入力層が「未到着バイトに対して EOF を返さず待つ」ことができれば、
アップロード中のファイルでもデコードが開始・継続できるはず。
開始できる時点はコンテナ形式（moov の位置など）で変わるはず。

## 結論（要約）

- **仮説は成立した。** 待てる入力（カスタム AVIO）を使えば、3形式すべてで
  アップロード中にデコードを開始し、最後まで（ベースラインと同フレーム数）完走できた。
- **開始時点は形式で大きく違う。** moov が先頭にある MP4 と MKV は最初のチャンク
  （全体の 5.5%）で初フレームが出た。moov が末尾にある MP4 は、moov が届く
  アップロード 100% 時点まで開始できなかった（デコーダが末尾へ seek して待ち続ける）。
- **待てない通常入力ではダメ。** 40% で切った静的ファイルを開くと、MP4(faststart) は
  途中までデコードしてエラー、MP4(tail) はオープン自体が失敗、MKV はエラーにならず
  **無言で途中終了**（142/360 フレーム）した。「待てる入力」が要る裏付け。

## 実験条件

- サンプル: ffmpeg CLI 8.1.2 で生成。testsrc2、12秒、640x360、30fps、H.264 (libx264)、音声なし。
  - `samples/faststart.mp4`（`-movflags +faststart`、1,198,071 bytes、moov は offset 32 から 4,928 bytes）
  - `samples/tail.mp4`（通常、1,198,071 bytes、moov は offset 1,193,143 から末尾まで 4,928 bytes）
  - `samples/sample.mkv`（1,196,303 bytes）
  - 再生成コマンドは本ファイル末尾。
- アップロード模擬: 別スレッドが元ファイルを **64 KiB チャンク / 40ms 間隔**で
  部分ファイルへ順次コピー（全体約1.2MB → 19チャンク、約0.76秒で完了）。
  `Mutex + Condvar` で「先頭から連続して到着済みのバイト数」と「完了フラグ」を公開。
  総サイズは開始時点で既知（Esotereel の `BeginAssetUpload { total_size }` と同じ前提）。
- デコード: ffmpeg-next 9.0。ビデオストリームを H.264 デコードしてフレーム数を数える。
- 計測: 初フレームがデコーダから出た瞬間のアップロード進捗（到着済みバイト / 総サイズ）と経過時間。

## 使ったカスタム IO API（ffmpeg-next 9.0 の正確な形）

`format::io::IO` トレイトのようなものは 9.0 には無く、**`StreamIo`** が正体だった
（`src/format/context/stream_io.rs`）。

```rust
use ffmpeg_next::format::context::StreamIo;

// T: Read + Seek + Send + 'static な自前ストリームを AVIOContext 化する
let io = StreamIo::from_read_seek(growing_file)?;
// filename は形式判定のヒント。options は Option<Dictionary>
let input = ffmpeg_next::format::input_from_stream(io, Some("faststart.mp4"), None)?;
```

要点（クレートのドキュメント/ソースで確認）:

- ストリームは **blocking でなければならない**。`WouldBlock` を返すとコンテキストが
  毒されて復旧しない。だから「待つ」処理は `Read` 実装の中で Condvar ブロックするのが正解。
- `read` が `Ok(0)` を返すと `AVERROR_EOF` として扱われる。これが「本物の EOF」の伝え方。
- seek は std の `Read + Seek` に変換される。`AVSEEK_SIZE` はクレート側が
  `stream_position()` + `seek(SeekFrom::End(0))` で処理するので、
  **`SeekFrom::End` が「最終的な総サイズ」を返せば、デコーダはアップロード中でも
  ファイル全体のサイズを知ることができる**（今回の実装はこれを利用）。
- キャンセルしたくなったら `format::input_from_stream_with_interrupt` があり、
  interrupt コールバックでブロック中の読みを `Error::Exit` で中断できる。

## GrowingFile の実装方針（src/main.rs）

- `read`: 現在位置のバイトが1つでも到着するまで Condvar で待つ。到着済み範囲内だけを
  `read_at` で読み、短く返す（AVIO 側のバッファリングに任せる）。
  完了フラグが立ち、かつ到着済み範囲を読み尽くしたら初めて `Ok(0)` = EOF。
- `seek`: 位置は論理オフセットで管理し、`End` は総サイズ基準。到着済み範囲の外への
  seek も許可し、その後の `read` が到着を待つ（tail.mp4 の moov 読みはこの経路）。
- uploader が（完了フラグを立てずに）死ぬと read は永遠に待つ。実運用では
  切断時に完了フラグを立てるか、interrupt で中断する仕組みが必須。

## 結果

| 形式 | ベースライン | growing 総フレーム | 初フレーム時点の進捗 | 初フレーム時刻 | growing 所要 |
|---|---|---|---|---|---|
| faststart.mp4 (moov 先頭) | 360 | **360（完走）** | **5.5%**（65,536 / 1,198,071 B） | 12.1 ms | 735 ms |
| tail.mp4 (moov 末尾) | 360 | **360（完走）** | **100.0%**（全量到着後） | 737.5 ms | 1,072 ms |
| sample.mkv | 360 | **360（完走）** | **5.5%**（65,536 / 1,196,303 B） | 8.3 ms | 768 ms |

対照実験（40% で切った静的ファイルを通常の `format::input` で開く）:

| 形式 | 結果 |
|---|---|
| faststart.mp4 | オープン成功、139 フレームまでデコード、その後 `Invalid data found when processing input` でエラー終了 |
| tail.mp4 | **オープン失敗**（`moov atom not found`、同上エラー） |
| sample.mkv | オープン成功、142 フレームで「ストリーム終了」扱い。**エラーにならず無言で途中まで** |

## できなかったこと・注意点

- **moov 末尾 MP4 は早く見せられない。** 入力層の問題ではなく、デコード開始に必要な
  索引自体がファイル末尾にあるため。アップロードする側が faststart で作るか、
  形式変換しない限り、この形式では「完了前に見せる」は実質できない
  （届いた瞬間に開始はするので、完了処理を待つよりは僅かに早い）。
- **シークは未検証。** 今回は先頭からの逐次デコードのみ。受信済み範囲の外への
  シークは「read が到着を待つ」実装にはなっているが、MKV の cues のように
  索引が末尾にある形式では、シーク解決自体が末尾到着待ちになる可能性がある。
- **到着は「先頭から連続」が前提。** チャンクが順不同に届く場合、公開すべき
  `available` は「連続した接頭辞の長さ」でなければならない（穴の先を available に
  すると read がゴミを読む）。Esotereel の UploadManager は現状 received を
  最大到達点で管理しているので、ストリーミング対応時は接頭辞管理への変更が必要。
- **中断処理は未実装。** アップロード側の切断・失敗時に、待っているデコーダを
  起こす経路（完了フラグ or interrupt）が要る。

## Esotereel の VideoStreamer へ統合する場合の変更点（見立て）

現行: `lib/src/decode/videostreamer.rs` の `VideoStreamer::new(path)` が
`ffmpeg::format::input(&path)` で完成ファイルを開き、`core` の `InitStream { path }` が起点。

1. **入力ソースの抽象化**: `VideoStreamer` に `StreamIo` から開くコンストラクタ
   （例: `new_from_stream(io, filename_hint)`）を足し、パス入力と共存させる。本体は
   `Input` を保持するだけなので差分は小さい見込み。
2. **アップロードセッションとの接続**: core 側で、アップロード中アセットの
   ストリーム要求が来たら、UploadManager のセッション状態（総サイズ・到着済み
   接頭辞・完了フラグ・Condvar）を共有する GrowingFile 相当を構築して渡す。
   セッション側はチャンク到着ごとの notify と、完了/中断時のフラグ立てが必要。
3. **完了後の扱い**: 完了フラグが立てば GrowingFile は通常ファイルと等価に振る舞う
   ので、同じクリップ・アセット ID のまま移行できる。新規ストリームは通常パスで
   開き直してもよい。
4. **形式の期待値管理**: faststart でない MP4 は早く見せられない、という制約は
   UI 側の期待値として明示すべき（「索引到着待ち」表示など）。
5. **中断**: クライアント切断時は interrupt（`input_from_stream_with_interrupt`）か
   完了フラグでデコーダを解放する。現行のストリーム破棄フローと合わせて設計する。

## サンプル再生成コマンド

```sh
cd samples
ffmpeg -f lavfi -i testsrc2=duration=12:size=640x360:rate=30 -c:v libx264 -pix_fmt yuv420p -movflags +faststart faststart.mp4
ffmpeg -f lavfi -i testsrc2=duration=12:size=640x360:rate=30 -c:v libx264 -pix_fmt yuv420p tail.mp4
ffmpeg -f lavfi -i testsrc2=duration=12:size=640x360:rate=30 -c:v libx264 -pix_fmt yuv420p sample.mkv
```

## 実行方法

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo run
```
