# プラグイン開発ガイド: チュートリアル（新しいクリップ種別を追加する）

## 概要

このチュートリアルでは、標準プラグイン（`plugins/std/`）の video クリップを題材に、新しいプラグインとして新しいクリップ種別を追加する手順を説明します。

先に [概要](overview.md) と [コンポーネント定義リファレンス](components.md) を読んでおくことをおすすめします。

## 1. 題材: 標準プラグインの video クリップ

video クリップは、次のファイルの組み合わせでできています。

| ファイル | 役割 |
|----------|------|
| `plugins/std/manifest.toml` | プラグイン `std` の宣言 |
| `plugins/std/clips/video.toml` | クリップ種別 `std:video` の宣言 |
| `plugins/std/components/video.toml` | コンポーネント `std:video` と子 `std:video-transform` の定義 |
| `plugins/std/components/video.rhai` | `std:video` の描画スクリプト |

ポイントは 2 つです。

1. **クリップ種別とコンポーネントが同じ ID（`std:video`）で結びつく**こと。プロパティと描画はコンポーネント側が持ちます
2. **描画は capability ベース**であること。video コンポーネントは `frame-provider` を宣言し、Scheduler が標準関数 `render_frame` を呼びます。スクリプトは `ctx.get_video_texture()` でテクスチャを受け取り、`ctx.render_video(...)` で描画します

## 2. 新しいプラグインを作る

ここでは、プラグイン `mypack` に video と同種のメディアクリップ `photo` を追加する例で手順を示します。

### 手順

1. プラグインディレクトリを作成する
   - 配置場所は、組み込み扱いならインストールディレクトリ配下の `std_plugins/`、ユーザー追加ならユーザーデータディレクトリ配下の `plugins/` です
   - リポジトリ内では `plugins/` 配下に置きます

   ```
   plugins/mypack/
   ├── manifest.toml
   ├── clips/
   │   └── photo.toml
   └── components/
       ├── photo.toml
       └── photo.rhai
   ```

2. `manifest.toml` を書く

   ```toml
   id = "mypack"
   name = "My Plugin Pack"
   version = "0.0.1"
   ```

   `id` がすべての名前空間付き ID の接頭辞になります。

3. クリップ種別を宣言する（`clips/photo.toml`）

   ```toml
   [[kinds]]
   id = "photo"
   render_script = "render_clip_photo"
   tags = ["media"]
   ```

4. ルートコンポーネントを定義する（`components/photo.toml`）
   - クリップ種別と**同じローカル ID**（`photo`）にします
   - メディアを扱うので `frame-provider` と `prefetchable` を宣言し、video と同じパターンでメディアプロパティと変換プロパティを持たせます

   ```toml
   [[components]]
   id = "photo"
   name = "Photo"
   kind = "media-provider"
   capabilities = ["frame-provider", "prefetchable"]
   editor = "video"
   script = "photo.rhai"
   properties = [
     { key = "photo:media", label = "Media",
       kind = { type = "group", reference = "photo:media" },
       fields = [
         { key = "source", label = "Source", kind = { type = "string" }, default = "" },
         { key = "source_offset", label = "Source Offset", kind = { type = "string" }, default = "0" },
       ],
       capability = "frame-provider",
       member_capabilities = { source = "droppable", source_offset = "media-offset-provider" },
       default = { source = "", source_offset = "0" } },
     { key = "photo:transform", label = "Transform",
       kind = { type = "map", value_kind = { type = "array", item_kind = { type = "float", step = 0.1 } },
              known_keys = ["position", "rotation", "scale"] },
       capability = "transformable",
       default = { position = [0.0, 0.0, 0.0], rotation = [0.0, 0.0, 0.0], scale = [1.0, 1.0, 1.0] } },
   ]
   ```

   プロパティのキーはプラグイン内で一意にします。この例では `photo:media`、`photo:transform` とし、名前空間付き ID は `mypack:photo:media` などになります。

5. 描画スクリプトを書く（`components/photo.rhai`）
   - 宣言した capability に対応する標準関数を実装します

   ```rhai
   fn render_frame(ctx, props) {
       let tex = ctx.get_video_texture();
       let transform = props["mypack:photo:transform"];
       ctx.render_video(tex, transform.as_mat4());
   }

   fn prefetch(ctx, props) {
   }
   ```

   `props` のキーには**自分のプラグイン ID を含む完全な名前空間付き ID**を使う点に注意してください。

6. アプリを起動し、プラグインが読み込まれることを確認する
   - 起動ログに `Loaded X/Y plugins successfully` と `Plugin indices rebuilt` が出力されます
   - 読み込みに失敗したプラグインはログにエラーが残り、そのプラグインだけが除外されます。他のプラグインやアプリ全体は止まりません
   - タイムラインに新しいクリップ種別が追加でき、メディアを指定して描画されれば成功です

## 3. よくあるつまずき

- **クリップは追加できるが描画されない**: クリップ種別と同 ID のコンポーネントがあるか、`frame-provider` を宣言しているか、標準関数 `render_frame` を実装しているかを確認してください
- **スクリプトの関数が見つからないとエラーになる**: capability に対応する標準関数はすべて実装が必要です。使わない場合も `prefetch` のように空の関数を置いてください
- **読み込み時に重複エラーになる**: ID は全プラグインを通じて一意です。クリップ種別・コンポーネント・プロパティキー・ツールバーボタンの重複を確認してください
- **関数の重複エラーになる**: 同じ基準ディレクトリ（`components/` やプラグインルート）配下の `.rhai` ファイルはすべてマージされます。同名・同引数の関数の重複を避けてください
- **`props` にキーが見つからない**: `props` にはそのコンポーネントが宣言したプロパティだけが、完全な名前空間付き ID をキーとして渡されます

## 4. 次のステップ

- 設定項目を追加したい場合: `settings.toml` に `[[fields]]` を書きます（[スクリプト API リファレンス](script-api.md) 第 5 節）
- ツールバーボタンを追加したい場合: `toolbars.toml` とボタン用スクリプトを用意します（同第 5 節）
- コンポーネントを階層化したい場合: `children` で子コンポーネントを参照します（[コンポーネント定義リファレンス](components.md) 第 4 節）
