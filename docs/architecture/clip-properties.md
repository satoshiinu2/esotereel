# クリップの共有プロパティとモデル行列

## 1. 名前付きプロパティ

クリップ種別は `clips/` 直下の TOML ファイルに宣言し、プロパティ定義は
`components/` 直下のコンポーネントが持つ。クリップ種別は `components` フィールドで
構成コンポーネントを宣言し、プロパティは構成全体から合成される。
全プラグインの読み込み後に参照を解決するため、ファイルの読み込み順には依存しない。
プロパティはコンポーネントが宣言する能力（`ComponentCapability`）から参照し、
プロパティアドレス（`std:media#media` など）をコード中にハードコードしてはならない。
プロパティは「所有コンポーネントの名前空間付き ID + コンポーネント内ローカル名」で
識別し、正規形は `plugin:component#property/member` とする。`:` は名前空間付き ID、
`#` はプロパティ、 `/` はメンバーの区切りで、`.` はサブモジュール用に予約する。
所有者が文脈から明らかな場所では相対形 `:#property/member` も使える
（先頭の `:` が所有者の省略＝self を明示する）。

```toml
# components/media.toml
[[components]]
id = "media"
name = "Media"
kind = "media-provider"
capabilities = ["frame-provider", "prefetchable", "file-drop-handler"]
editor = "video"
script = "video.rhai"
properties = [
  { key = "media", label = "Media", kind = { type = "group" }, fields = [ { key = "source", label = "Source", kind = { type = "string" }, default = "" }, { key = "source_offset", label = "Source Offset", kind = { type = "string" }, default = "0" } ], capability = "frame-provider", member_capabilities = { source = "media-source", source_offset = "media-offset-provider" }, default = { source = "", source_offset = "0" } },
]

[[components.requires]]
name = "transform"
capability = "transformable"
required = true

# components/transform.toml
[[components]]
id = "transform"
name = "Transform"
kind = "transform"
capabilities = ["transformable"]
editor = "transform"
properties = [
  { key = "transform", label = "Transform", kind = { type = "group" }, fields = [ { key = "position", label = "Position", kind = { type = "array", item_kind = { type = "float", step = 0.1 } }, default = [0.0, 0.0, 0.0] }, { key = "rotation", label = "Rotation", kind = { type = "array", item_kind = { type = "float", step = 0.1 } }, default = [0.0, 0.0, 0.0] }, { key = "scale", label = "Scale", kind = { type = "array", item_kind = { type = "float", step = 0.1 } }, default = [1.0, 1.0, 1.0] } ], capability = "transformable", default = { position = [0.0, 0.0, 0.0], rotation = [0.0, 0.0, 0.0], scale = [1.0, 1.0, 1.0] } },
]

# clips/video.toml
[[kinds]]
id = "video"
render_script = "render_clip_video"
tags = ["media"]
components = ["media", "transform"]
```

`capability` フィールドは、このプロパティがどのコンポーネント能力に紐付くかを示す。
レンダラー等は `FrameProvider` 能力を持つプロパティからメディア情報を、
`Transformable` 能力を持つプロパティから変換情報を逆引きする。

グループ型プロパティのメンバー（`source`, `source_offset` 等）もまた、`member_capabilities`
で能力を紐付けられる。レンダラーはハードコードされたメンバキー文字列ではなく、
`member_capabilities` から能力ベースでメンバキーを逆引きする。これによりプラグインは
任意のメンバー名を宣言できる。

グループ内のキーはローカル名で扱う。異なるグループには同じキーを配置できるが、
同じグループ内の重複は拒否する。未定義参照、参照の循環、32グループを超える深さも拒否する。
未使用の定義も読み込み時に検証する。
並びは種別が宣言するコンポーネントの順、各コンポーネントの `properties` の順、
その後に各定義の `fields` の順となる。

### 入れ子の定義参照

次の例は将来のテロップ生成プラグインで使える定義例であり、テロップ描画機能そのものは未実装である。
`text:text` はテキストクリップ側にも同じ名前で定義できる。

```toml
# text プラグイン側の clips/text.toml
[[properties]]
name = ["text:text"]
label = "Text"

[[properties.fields]]
key = "content"
label = "Content"
kind = { type = "string" }

# generator プラグイン側の clips/telop.toml
[[kinds]]
id = "telop"
render_script = "render_telop"
properties = ["generator:telop"]

[[properties]]
name = ["generator:telop"]
label = "Telop"

[[properties.fields]]
key = "caption"
label = "Caption"
kind = { type = "group", reference = "text:text" }
default = { content = "Caption" }

[[properties.fields]]
key = "header"
label = "Header"
kind = { type = "group", reference = "text:text" }
default = { content = "Header" }
```

`caption` と `header` は同じ定義を参照するが、値はそれぞれ独立する。
参照先のグループにも `type = "group"` の項目を定義できる。
参照ごとの `default` は指定したメンバーを上書きし、未指定メンバーには定義元の初期値を使う。
未定義メンバーや型が異なる初期値は拒否する。

Rhai は `props["generator:telop"].caption.content` のようにアクセスする。
生成処理を実装する際は、この値を生成先のテキストプロパティへ渡せる。
別クリップの現在値を追従する動的バインディングは未実装であり、定義参照とは区別する。
将来は `ClipId` と `PropertyPath` を値の参照先として使い、参照の評価・循環検査を別途行う。

### 保存・編集・GUI

スキーマは `FieldTypeKind::Group { fields }` と
`FieldTypeKind::Map { value_kind, known_keys }` のツリーとして保持し、
値は `FieldValue::Map` を使って同じ階層で保存する。
ルートは所有コンポーネントの ID とプロパティ名（`std:media#media`）、
グループ内のメンバーは `/` 区切りのローカル名となる。
例: `std:media#media/source_offset`、`std:transform#transform/scale`。

編集対象は型付き `PropertyPath` で指定する。文字列表現は `std:transform#transform/scale` や
`generator:telop#caption/content`。ラベルや表示カテゴリを変更してもパスは変わらない。
コマンドは末端の値だけを更新し、Undo/Redo もその末端に限定するため、兄弟の値を上書きしない。
受信側への通知では更新後のルートグループを送る。

C ABI の引数とスキーマ構造体は変更せず、GUI用にのみ末端項目へ展開する。
GUI はルートグループのタブと再帰的な折りたたみセクションを表示する。
複数クリップの共通項目は、グループ全体ではなく末端のパスと型で判定する。
同じ `transformable` 能力を持つプロパティでも、`std:transform#transform/scale` と
`text:transform#transform/scale` のようにパスが異なる項目は一括編集の対象にならない。

旧プロパティ値の自動移行は未実装。以前の `std:video/media/source` や `std:video/transform/scale` は、
それぞれ `std:media#media/source`、`std:transform#transform/scale` に変わる。
標準動画スクリプトは今回のアドレス形式を前提とする。
通信コマンドのキー型は `NamespacedID` から `PropertyPath` に変わるため、クライアントとサーバーは
同じ版を使う必要がある。

## 2. 他プラグインからの参照

他プラグインの定義も名前空間付き ID を `properties` に指定して参照できる。
定義元の名前空間はそのまま維持し、参照元のプラグイン名には置き換えない。
参照解決は `ClipKind::resolve_property_reference` に集約しており、参照元の種別 ID と
定義元のプロパティ ID の両方を渡す。将来の権限判定はこの境界に追加する。

現時点では参照の許可・拒否は行わない。プラグインの認証、ユーザー承認、
ファイルアクセスやスクリプト実行を含む権限管理は未実装である。
他プラグインの名前空間を使ったプロパティ定義の作成は拒否する。

## 3. 動画サイズと変換

表示サイズは `std:transform#transform/scale` にピクセル単位で指定する。
`std:width` と `std:height` は使用しない。Transform プロパティは `std:transform` の
`transform` に `capability = "transformable"` を付けて宣言する。

`scale` の X・Y は倍率ではなく描画幅・高さ（ピクセル単位）を表し、表示サイズはこの値だけで決まる。
元動画の解像度は表示サイズに自動反映しないため、元動画と異なる縦横比も指定できる。
例えば `[544.0, 408.0, 1.0]` は 544 × 408 ピクセル、`[272.0, 204.0, 1.0]` は
272 × 204 ピクセルで描画する。新規クリップの暫定初期値は、動画の解像度によらず
`[100.0, 100.0, 1.0]`。

| フィールド | 内容 | 既定値 |
| --- | --- | --- |
| `std:transform#transform/position` | XYZ 位置 | `[0.0, 0.0, 0.0]` |
| `std:transform#transform/rotation` | XYZ 回転、度単位 | `[0.0, 0.0, 0.0]` |
| `std:transform#transform/scale` | 描画幅・高さ（ピクセル）と Z スケール | `[100.0, 100.0, 1.0]` |

GUI は数値配列を各成分の数値欄として編集する。数値欄は暫定的に ±1,000,000,000 の範囲を持つ。
スキーマの min/max/step を FFI 経由で編集UIに反映する処理は未実装である。

Rhai の `props["transform"].as_mat4()` は Transform 型の値から
`Mat4` を生成する。各ベクトルは有限な数値を3個含む必要があり、回転順は `EulerRot::XYZ`。
`ctx.set_transform(matrix)` はモデル行列を置き換える。
`ctx.apply_transform(matrix)` は現在のモデル行列の右側に行列を合成する。

標準動画スクリプトは `ctx.render_video(texture_id, transform)` を呼び出し、Transform
プロパティから得た行列を明示的に描画へ渡す。Rust 側は動画の中心を原点にした
単位矩形を描画し、メタデータでストリームが利用可能なことを確認する。
デコードフレームを取得できるまでは描画をスキップする。
動画の矩形は中心を原点とした単位サイズで保持し、モデル行列は `T × R × S` とする。
引数で矩形を指定する既存の `ctx.render_video(texture_id, x, y, width, height)` も維持する。
最終変換は既存シェーダーで `P × V × M × vertex` として計算する。
クリップ変換はプロパティとして管理し、標準動画プラグインは
`transform` プロパティ（`capability = "transformable"`）の行列を `render_video` へ明示的に渡す。
他のプラグインも `set_transform` または `apply_transform` を利用できる。
以前の `std:width` / `std:height` の値が
既存クリップに残っていても描画には使わず、`std:transform#transform/scale` の寸法を基準にする。

標準動画スクリプトは `ctx.get_video_texture()` を使う。動画ソース、取得時刻、変換行列は
Rust 側の型付きプロパティビューで処理し、`(クリップ内の経過 tick +
std:media#media/source_offset) / ticks_per_second` を取得時刻とする。
プロパティのパス・型変換は Rust 内で集約し、GUI は引き続き TOML スキーマから生成する。

## 4. 動画取得と描画

描画スクリプトが必要な動画と時刻を記録し、テクスチャ更新は記録を消さずに参照する。
その後に Rust の GUI ヘルパーが初期化・パケット取得要求を送信する。
停止中のプレビューも描画周期から要求を送るため、タイムライン操作や再生開始を必要としない。

## 5. 通信時のプロパティキー

プロパティマップのキーに使う `PropertyKey` は、通常型・rkyv のアーカイブ型ともに
所有者の `full` とローカル名の組で等価比較とハッシュ計算を行う。両型で計算対象が
異なると、受信時にハッシュマップの位置検証が失敗し、`AddClip` が
`InvalidKeyPosition` で拒否される。
通信の検証は維持し、送受信のキーの計算を一致させる。

## 6. 描画の検証

通常のテストでは、指定したピクセル寸法で描画され、頂点がカメラの描画範囲に投影されることを確認する。
GPU を使う検証は次のコマンドで明示的に実行する。テクスチャ転送から描画、
画素の読み戻しまでを通し、赤・緑の動画領域と背景色を確認する。

```bash
cargo test -p esotereel_lib --lib video_preview_renders_colored_pixels -- --ignored --nocapture
```

実ファイルの5秒地点をパケット取得・デコードして描画し、読み戻した画素と比較する検証も用意している。
次の環境変数で対象動画を指定する。

```bash
ESOTEREEL_TEST_VIDEO=/path/to/video.mp4 cargo test -p esotereel_lib --lib video_file_preview_matches_decoded_frame -- --ignored --nocapture
```

クリップ追加の仮実装は長さが 50 tick 固定で、60 tick/秒のタイムラインでは約 0.83 秒となる。
先頭が黒い動画では、その区間だけを再生しても映像が現れない場合がある。
実ファイルの後の位置を確認するには、クリップの右端を延ばして再生位置を移動する。
