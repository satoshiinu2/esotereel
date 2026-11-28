# プラグイン開発ガイド: コンポーネント定義リファレンス

## 概要

コンポーネントは、クリップの描画・振る舞いとプロパティを定義する部品です。`components/` ディレクトリ配下の TOML ファイルに `[[components]]` として宣言し、あわせて Rhai スクリプトで振る舞いを実装します。1 ファイルに複数のコンポーネントを書くこともできますが、標準プラグインでは再利用しやすいよう 1 ファイル 1 コンポーネントに分割しています。

標準プラグインの `components/media.toml`、`components/transform.toml`、`components/video.toml` を例に、各要素を解説します。

```toml
# components/media.toml
[[components]]
id = "media"
name = "Media"
capabilities = ["prefetchable", "file-drop-handler", "frame-provider"]
editor = "video"
script = "media.rhai"
properties = [
  { key = "media", label = "Media",
    kind = { type = "group" },
    fields = [
      { key = "source", label = "Source", kind = { type = "string" }, default = "", capability = "media-source" },
      { key = "source_offset", label = "Source Offset", kind = { type = "string" }, default = "0", capability = "media-offset-provider" },
    ],
    capability = "frame-provider",
    default = { source = "", source_offset = "0" } },
]

[[components.requires]]
name = "transform"
capability = "transformable"
required = true

# components/transform.toml
[[components]]
id = "transform"
name = "Transform"
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

# components/video.toml
[[components]]
id = "video"
name = "Video"

[[components.requires]]
name = "media"
capability = "frame-provider"
required = true

[[components.requires]]
name = "transform"
capability = "transformable"
required = true
```

## 1. コンポーネント定義のフィールド

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `id` | 文字列 | ○ | コンポーネントのローカル ID。名前空間付き ID は `<plugin_id>:<id>` になります |
| `name` | 文字列 | ○ | コンポーネントの表示名 |
| `capabilities` | 文字列の配列 | ― | このコンポーネントが提供する能力の一覧（第 2 節）。省略時は空 |
| `editor` | 文字列 | ― | GUI 側でこのコンポーネントの編集に使うエディタの識別子 |
| `script` | 文字列 | ― | このコンポーネント専用の Rhai スクリプト。`components/` ディレクトリからの相対パスで指定します。省略したコンポーネントはスクリプト実行の対象外になります（GUI 専用定義など） |
| `properties` | スキーマの配列 | ― | このコンポーネントが提供するプロパティスキーマ（第 5 節）。省略時は空 |
| `requires` | テーブルの配列 | ― | 他コンポーネントへの依存宣言（第 4.1 節）。省略時は空 |

## 2. capability（能力）

capability は「コンポーネントが何を提供できるか」の宣言です。本体（レンダラーや GUI）は、コンポーネントの ID ではなく capability を見て連携します。

標準の capability は次のとおりです。文字列は TOML に書く小文字 kebab-case 表記です。

| 文字列 | 意味 | 標準関数 | フックタイミング |
|--------|------|----------|------------------|
| `frame-provider` | 時刻に対応する映像フレームを供給する | `render_frame` | Evaluate |
| `audio-provider` | 音声データを供給する | `render_audio` | Evaluate |
| `transformable` | 位置・回転・拡大縮小を提供する | `get_transform` | Process |
| `timeline-positionable` | タイムライン上での位置・長さ編集を提供する | `get_timeline_position` | Process |
| `prefetchable` | データの先読みを提供する | `prefetch` | Prefetch |
| `keyframeable` | キーフレーム編集を提供する | `get_keyframes` | Process |
| `media-source` | メディアソースのファイルパスを提供する（主にマップ型プロパティのメンバーに対して使う） | ― | Evaluate |
| `media-offset-provider` | メディアオフセット（ソース時間オフセット）を提供する（主にメンバーに対して使う） | ― | Evaluate |
| `file-drop-handler` | ファイルドロップ時に初期プロパティを返す | `handle_file_drop` | Prepare |
| `corner-deformable` | 四隅オフセットによるクワッド変形を提供する（グループ型プロパティに対して使う）。変形は lib が compute シェーダ `std:corner_deform` で適用する | ― | ― |
| `corner-top-left` | 左上の隅オフセットを提供する（グループ型プロパティのメンバーに対して使う） | ― | ― |
| `corner-top-right` | 右上の隅オフセットを提供する（グループ型プロパティのメンバーに対して使う） | ― | ― |
| `corner-bottom-right` | 右下の隅オフセットを提供する（グループ型プロパティのメンバーに対して使う） | ― | ― |
| `corner-bottom-left` | 左下の隅オフセットを提供する（グループ型プロパティのメンバーに対して使う） | ― | ― |

- **標準関数がある capability を宣言した場合、スクリプトに対応する関数を実装する必要があります**。未実装だとスケジューラの呼び出しがエラーになり、そのクリップは描画されません
- 標準関数のない capability は、プロパティやメンバーへの目印として使います（第 5 節の `capability`）
- 上記以外の文字列は書けません。未知の capability 文字列を指定すると、コンポーネント定義の読み込み時にエラーになります

## 3. フックと実行順序

コンポーネントの標準関数は、フックタイミングの順序で実行されます。順序は `ComponentHook` の宣言順で固定です。

```
Prepare → Prefetch → Evaluate → Process → Finalize
```

現在の実装で標準関数が割り当てられているのは Prefetch（`prefetch`）、Evaluate（`render_frame`、`render_audio`）、Process（`get_transform`、`get_timeline_position`、`get_keyframes`）です。Prepare と Finalize に対応する capability は現時点では存在しません。

描画時の実行は `Scheduler`（`lib/src/plugin/clip/component.rs`）が行います。

1. クリップ種別が宣言するコンポーネント列を、宣言順に実行対象コンポーネントの一覧とする（第 4 節）
2. コンポーネントごとに、スクリプトへ渡す `props` を構築する。`props` はサンドボックス化されており、自コンポーネントのプロパティ（`:#` 形式で参照）と、`[[requires]]` で宣言した依存先の capability に紐づくプロパティ（宣言名で参照）だけが解決できます（第 4.1 節）。兄弟コンポーネントのプロパティは、依存を宣言しない限り参照できません
3. capability をフックタイミング順に並べ替え、対応する標準関数を順に呼び出す

> **注意**: 現在の Scheduler は標準関数の戻り値を破棄します。描画結果は、後述する描画コンテキスト（`ctx`）へのメソッド呼び出しという副作用で返してください（[スクリプト API リファレンス](script-api.md)）。

## 4. コンポーネント構成の解決

どのコンポーネントでクリップ種別を構成するかは、クリップ種別側の `components` フィールドが宣言します（第 6 節）。各要素には、ローカル ID または `plugin:local` 形式の名前空間付き ID を書けます。実行時には、宣言された順序のままフラットな実行リストに展開されます。

- 同じコンポーネントが複数回書かれても、実行リストには 1 回だけ現れます（最初の出現位置が優先されます）
- 存在しないコンポーネントを書いた場合、解決時にエラーになります

`plugins/std` の例では、クリップ種別 `video` が `components = ["media", "transform", "video"]` を宣言します。`video` コンポーネントは自プロパティを持たず、描画スクリプトと `frame-provider` 能力を持ち、`[[requires]]` で media と transform に依存します。`transform` コンポーネント自体はスクリプトを持たず、`transformable` 能力と GUI エディタ（`transform`）の宣言、および変換プロパティを提供します。コンポーネントは特定のクリップ種別に属さない独立した部品なので、他のクリップ種別からも同じコンポーネントを構成に含めて再利用できます。

### 4.1 依存宣言（`[[requires]]`）

コンポーネントのスクリプトが他コンポーネントのプロパティを必要とする場合、`[[requires]]` で依存を宣言します。依存先はコンポーネント ID ではなく capability で解決されます。

```toml
[[components.requires]]
name = "transform"
capability = "transformable"
required = true
```

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `name` | 文字列 | ○ | 依存先の値を `props` で参照するときのローカル名。自コンポーネントのプロパティ名と重なるとエラーになります。requires 同士の重複もエラーです |
| `capability` | 文字列 | ○ | 依存する capability。構成コンポーネントの中から、この capability を宣言するプロパティ（またはメンバー、またはコンポーネント自体）が解決されます |
| `required` | 真偽値 | ― | 既定は `false`（任意依存）。`true` の場合、kind の構成で依存が満たせなければ定義の解決時にエラーになります。任意依存が満たせない場合は、その名前へのアクセスが `()` を返します（`props.has()` で有無を確認できます） |

`props` で参照できるのは、依存先の全プロパティではなく、宣言した capability に紐づく値だけです。プロパティが capability を宣言していればそのプロパティの値全体が、グループのメンバーが宣言していればそのメンバーの値だけが、`name` で参照できます。コンポーネント自体が capability を持つ場合は、参照できる値はありません（必須依存の充足だけが意味を持ちます）。

## 5. プロパティスキーマ

`properties` の各要素はプロパティスキーマで、クリップのプロパティ（GUI で編集され、スクリプトに渡される値）の型と既定値を宣言します。設定項目（`settings.toml`）も同じスキーマ形式を使います。

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `key` | 文字列 | ○ | プロパティのローカル名。プロパティは「所有コンポーネントの名前空間付き ID + ローカル名」で識別され、正規形のアドレスは `std:media#media` のようになります（第 5.3 節）。`:` `#` `/` は使えません |
| `label` | 文字列 | ○ | GUI に表示するラベル |
| `kind` | テーブル | ○ | 値の型（第 5.1 節） |
| `category` | 文字列の配列 | ― | GUI での分類。省略時は空 |
| `default` | 任意の TOML 値 | ― | 既定値。省略した場合は `kind` から導出されます（第 5.2 節） |
| `capability` | 文字列 | ― | このプロパティ（グループの場合はそのメンバー）が提供する能力。本体はこの宣言を使ってプロパティやメンバーを逆引きします |
| `fields` | スキーマの配列 | ― | `group` 型の場合の子フィールド定義 |

同じ一覧内での `key` の重複はエラーになります。

### 5.1 kind の型一覧

`kind` は `{ type = "...", ... }` 形式のテーブルで書きます。

| `type` | 追加パラメータ | 値の形式 | 備考 |
|--------|----------------|----------|------|
| `bool` | ― | 真偽値 | |
| `int` | `min`, `max`（任意） | 整数 | 範囲外の値はエラー |
| `float` | `step`（必須）, `min`, `max`（任意） | 小数 | 整数を書いても float として解釈されます |
| `string` | ― | 文字列 | |
| `enum` | `options`（必須） | 選択肢のうち 1 つ（文字列） | `options` が空だとエラー |
| `filepath` | `extensions`（任意） | 文字列、または文字列の配列 | ファイルパス。値はパスの集合として保持されます |
| `color` | ― | `"#RRGGBB"` または `"#RRGGBBAA"` 文字列 | |
| `array` | `item_kind`（必須） | 要素の配列 | `item_kind` には任意の kind を再帰的に書けます |
| `map` | `value_kind`（必須）, `known_keys`（任意） | キー → 値のテーブル | `known_keys` を指定すると、それ以外のキーは既定値の変換時に読み飛ばされます |
| `group` | `fields` | 子フィールドをまとめたテーブル | グループはフィールド宣言だけで構成されます。宣言されていないメンバーを含む値はエラーになります |

### 5.2 既定値の導出

`default` を省略した場合、`kind` から次のように導出されます。

| 型 | 導出される既定値 |
|----|------------------|
| `bool` | `false` |
| `int` / `float` | `0`（範囲に収まらない場合は最も近い境界値） |
| `enum` | `options` の先頭 |
| `string` / `filepath` | 空文字列 |
| `color` | `#FFFFFF` |
| `array` | 空の配列 |
| `map` | 空のテーブル |
| `group` | 各子フィールドの既定値を集めたテーブル |

### 5.3 プロパティと capability の紐付け

本体はプロパティを ID のハードコードではなく capability から解決します。プラグイン側では、役割を持つプロパティに `capability` を宣言してください。

`plugins/std` の例（コンポーネントは第 4 節のとおり media / transform に分かれています）:

- media コンポーネントのプロパティ `media`（group）に `capability = "frame-provider"` を宣言し、このプロパティがメディア情報の提供者であることを示す
- そのメンバーは各フィールド自身が `capability` を宣言する（`source` に `"media-source"`、`source_offset` に `"media-offset-provider"`）
- transform コンポーネントのプロパティ `transform`（group。メンバーは `position` / `rotation` / `scale`）には `capability = "transformable"` を宣言する

レンダラー側は、`frame-provider` 能力を持つプロパティからメディア情報を読みます。ソースパスのメンバーキーは `media-source` 能力を宣言するメンバーから、オフセットのメンバーキーは `media-offset-provider` から解決されます。どちらかが宣言されていない場合、そのクリップは描画されません（警告ログが出力されます）。

#### プロパティアドレス

プロパティを一意に指す正規形は `plugin:component#property/member` です（例: `std:media#media/source`）。区切りの役割は次のとおりです。

- `:` — 名前空間付き ID（プラグイン:コンポーネント）の区切り
- `#` — その ID の中身（プロパティ）を指す
- `/` — プロパティ内のメンバー
- `.` — プラグイン名前空間内のサブモジュール用に予約されており、プロパティ記法では使いません

所有者が文脈から明らかな場所では、所有者部分を省略した相対形 `:#property/member` も使えます（先頭の `:` が所有者の省略＝self を明示します。例: file-drop ハンドラの戻り値で自コンポーネントのプロパティを指す場合）。スクリプトの `set_property` などでは正規形を使います（[スクリプト API リファレンス](script-api.md)）。

## 6. クリップ定義（clips/*.toml）

クリップ種別は `clips/` 配下の TOML に `[[kinds]]` として宣言します。

```toml
[[kinds]]
id = "video"
components = ["media", "transform", "video"]
render_script = "render_clip_video"
tags = ["media"]
```

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `id` | 文字列 | ○ | クリップ種別のローカル ID |
| `components` | 文字列の配列 | ― | この種別を構成するコンポーネントの ID 列（第 4 節）。宣言順に実行され、プロパティは全コンポーネント分が合成されます。省略時は空で、その種別には描画もプロパティも提供されません |
| `render_script` | 文字列 | ○ | スクリプト名を保持するフィールド（下記の注意を参照） |
| `tags` | 文字列の配列 | ― | 種別の分類タグ。省略時は空 |

> **注意**: 現在の実装では、描画はコンポーネントの capability に基づくスケジューラ経由で行われ、`render_script` は描画には使用されません。ただし TOML のフィールド自体は必須です。

プロパティはクリップ種別側には書きません。プロパティは各コンポーネントが自身のスキーマとして提供します。
