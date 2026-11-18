# プラグイン開発ガイド: チュートリアル（新しいクリップ種別を追加する）

## 概要

このチュートリアルでは、標準プラグイン（`plugins/std/`）の video クリップを題材に、新しいプラグインとして新しいクリップ種別を追加する手順を説明します。

先に [概要](overview.md) と [コンポーネント定義リファレンス](components.md) を読んでおくことをおすすめします。

## 1. 題材: 標準プラグインの video クリップ

video クリップは、次のファイルの組み合わせでできています。

| ファイル | 役割 |
|----------|------|
| `plugins/std/manifest.toml` | プラグイン `std` の宣言 |
| `plugins/std/clips/video.toml` | クリップ種別 `std:video` の宣言（構成コンポーネントの指定を含む） |
| `plugins/std/components/media.toml` | コンポーネント `std:media` の定義 |
| `plugins/std/components/transform.toml` | コンポーネント `std:transform` の定義 |
| `plugins/std/components/video.toml` | コンポーネント `std:video` の定義（media / transform を束ねる合成役） |
| `plugins/std/components/media.rhai` | `std:media` のスクリプト（素材系の標準関数と `render_frame`） |

ポイントは 2 つです。

1. **クリップ種別が `components` で構成コンポーネントを宣言する**こと。種別 `std:video` は `components = ["media", "transform", "video"]` と宣言し、プロパティと描画は media / transform が、合成は video が持ちます。コンポーネントは種別から独立した部品で、他の種別からも再利用できます
2. **描画は capability ベース**であること。media コンポーネントは `frame-provider` を宣言し、Scheduler が標準関数 `render_frame` を media の props で呼びます。media は `[[requires]]` で transform に依存し、スクリプトは `ctx.get_video_texture()` でテクスチャを受け取り、`ctx.render_video(...)` で描画します。video コンポーネントは media と transform への依存を宣言する合成役です

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
   components = ["photo", "photo-transform"]
   ```

4. コンポーネントを定義する（`components/photo.toml`）
   - メディア担当（`photo`）と変換担当（`photo-transform`）の 2 つのコンポーネントに分けます。手順 3 の `components` でこの 2 つを宣言しています（std の video では、メディア担当 `media` と変換担当 `transform` に合成役の `video` を加えた 3 部品にしています）
   - メディア担当は `frame-provider` と `prefetchable` を宣言しメディアプロパティを持ち、変換担当は `transformable` を宣言し変換プロパティを持ちます

   ```toml
   [[components]]
   id = "photo"
   name = "Photo"
   capabilities = ["frame-provider", "prefetchable"]
   editor = "video"
   script = "photo.rhai"
   properties = [
     { key = "media", label = "Media",
       kind = { type = "group" },
       fields = [
         { key = "source", label = "Source", kind = { type = "string" }, default = "" },
         { key = "source_offset", label = "Source Offset", kind = { type = "string" }, default = "0" },
       ],
       capability = "frame-provider",
       member_capabilities = { source = "media-source", source_offset = "media-offset-provider" },
       default = { source = "", source_offset = "0" } },
   ]

   [[components.requires]]
   name = "transform"
   capability = "transformable"
   required = true

   [[components]]
   id = "photo-transform"
   name = "Photo Transform"
   capabilities = ["transformable"]
   editor = "transform"
   properties = [
     { key = "transform", label = "Transform",
       kind = { type = "group" },
       fields = [
         { key = "position", label = "Position", kind = { type = "array", item_kind = { type = "float", step = 0.1 } }, default = [0.0, 0.0, 0.0] },
         { key = "rotation", label = "Rotation", kind = { type = "array", item_kind = { type = "float", step = 0.1 } }, default = [0.0, 0.0, 0.0] },
         { key = "scale", label = "Scale", kind = { type = "array", item_kind = { type = "float", step = 0.1 } }, default = [1.0, 1.0, 1.0] },
       ],
       capability = "transformable",
       default = { position = [0.0, 0.0, 0.0], rotation = [0.0, 0.0, 0.0], scale = [1.0, 1.0, 1.0] } },
   ]
   ```

   プロパティのキーはコンポーネント内のローカル名です。プロパティは所有コンポーネントの ID と組で識別され、この例のアドレスは `mypack:photo#media`、`mypack:photo-transform#transform` になります。メディア担当が変換担当のプロパティを使うため、`[[components.requires]]` で `transformable` への依存も宣言しています。

5. 描画スクリプトを書く（`components/photo.rhai`）
   - 宣言した capability に対応する標準関数を実装します

   ```rhai
   fn render_frame(ctx, props) {
       let tex = ctx.get_video_texture();
       let transform = props["transform"];
       ctx.render_video(tex, transform.as_mat4());
   }

   fn prefetch(ctx, props) {
   }
   ```

   `props` のキーは、依存については**宣言名**（裸の名前）、自コンポーネントのプロパティについては **`:#` を付けたローカル名**です。スクリプトから参照できるのは自コンポーネントのプロパティだけですが、`[[requires]]` で宣言した依存先については、その capability に紐づく値が宣言名で参照できます。この例では `photo-transform` の `transform` プロパティが、依存宣言の名前 `transform` で参照できます。

6. アプリを起動し、プラグインが読み込まれることを確認する
   - 起動ログに `Loaded X/Y plugins successfully` と `Plugin indices rebuilt` が出力されます
   - 読み込みに失敗したプラグインはログにエラーが残り、そのプラグインだけが除外されます。他のプラグインやアプリ全体は止まりません
   - タイムラインに新しいクリップ種別が追加でき、メディアを指定して描画されれば成功です

## 3. よくあるつまずき

- **クリップは追加できるが描画されない**: クリップ種別の `components` にメディア担当コンポーネントが含まれているか、そのコンポーネントが `frame-provider` を宣言しているか、標準関数 `render_frame` を実装しているかを確認してください
- **スクリプトの関数が見つからないとエラーになる**: capability に対応する標準関数はすべて実装が必要です。使わない場合も `prefetch` のように空の関数を置いてください
- **読み込み時に重複エラーになる**: ID は全プラグインを通じて一意です。クリップ種別・コンポーネント・プロパティキー・ツールバーボタンの重複を確認してください
- **関数の重複エラーになる**: 同じ基準ディレクトリ（`components/` やプラグインルート）配下の `.rhai` ファイルはすべてマージされます。同名・同引数の関数の重複を避けてください
- **`props` で宣言していない名前だとエラーになる**: `props` で参照できるのは、自コンポーネントのプロパティ（`:#` 形式）と `[[requires]]` で宣言した依存（宣言名）だけです。自プロパティを裸の名前で引いた場合もエラーになります（`:#` 形式へ誘導されます）。他コンポーネントの値を使う場合は依存を宣言しているか、その依存先がクリップ種別の構成に含まれているかを確認してください。任意依存の有無は `props.has()` で確認できます

## 4. 次のステップ

- 設定項目を追加したい場合: `settings.toml` に `[[fields]]` を書きます（[スクリプト API リファレンス](script-api.md) 第 5 節）
- ツールバーボタンを追加したい場合: `toolbars.toml` とボタン用スクリプトを用意します（同第 5 節）
- コンポーネントを組み合わせたい場合: クリップ種別の `components` にコンポーネント ID を並べて合成します（[コンポーネント定義リファレンス](components.md) 第 4 節）
