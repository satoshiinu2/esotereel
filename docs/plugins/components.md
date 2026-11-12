# プラグイン開発ガイド: コンポーネント定義リファレンス

## 概要

コンポーネントは、クリップの描画・振る舞いとプロパティを定義する部品です。`components/` ディレクトリ配下の TOML ファイルに `[[components]]` として宣言し、あわせて Rhai スクリプトで振る舞いを実装します。

標準プラグインの `components/video.toml` を例に、各要素を解説します。

```toml
[[components]]
id = "video"
name = "Video"
kind = "media-provider"
capabilities = ["frame-provider", "prefetchable"]
editor = "video"
script = "video.rhai"
children = ["video-transform"]
properties = [
  { key = "video:media", label = "Media",
    kind = { type = "group", reference = "video:media" },
    fields = [
      { key = "source", label = "Source", kind = { type = "string" }, default = "" },
      { key = "source_offset", label = "Source Offset", kind = { type = "string" }, default = "0" },
    ],
    capability = "frame-provider",
    member_capabilities = { source = "droppable", source_offset = "media-offset-provider" },
    default = { source = "", source_offset = "0" } },
  { key = "video:transform", label = "Transform",
    kind = { type = "map", value_kind = { type = "array", item_kind = { type = "float", step = 0.1 } },
           known_keys = ["position", "rotation", "scale"] },
    capability = "transformable",
    default = { position = [0.0, 0.0, 0.0], rotation = [0.0, 0.0, 0.0], scale = [1.0, 1.0, 1.0] } },
]

[[components]]
id = "video-transform"
name = "Transform"
kind = "transform"
capabilities = ["transformable"]
editor = "transform"
```

## 1. コンポーネント定義のフィールド

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `id` | 文字列 | ○ | コンポーネントのローカル ID。名前空間付き ID は `<plugin_id>:<id>` になります |
| `name` | 文字列 | ○ | コンポーネントの表示名 |
| `kind` | 文字列 | ○ | コンポーネントの種別への参照。ローカル名で書くと自プラグインの名前空間で解決され、`plugin:local` 形式で他プラグインの種別も参照できます |
| `capabilities` | 文字列の配列 | ― | このコンポーネントが提供する能力の一覧（第 2 節）。省略時は空 |
| `editor` | 文字列 | ― | GUI 側でこのコンポーネントの編集に使うエディタの識別子 |
| `script` | 文字列 | ― | このコンポーネント専用の Rhai スクリプト。`components/` ディレクトリからの相対パスで指定します。省略したコンポーネントはスクリプト実行の対象外になります（GUI 専用定義など） |
| `children` | 文字列の配列 | ― | 内部で使う子コンポーネントの ID（第 4 節）。省略時は空 |
| `properties` | スキーマの配列 | ― | このコンポーネントが提供するプロパティスキーマ（第 5 節）。省略時は空 |

## 2. capability（能力）

capability は「コンポーネントが何を提供できるか」の宣言です。本体（レンダラーや GUI）は、コンポーネントの `kind` や ID ではなく capability を見て連携します。

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
| `droppable` | ドラッグ & ドロップでファイルパスを書き込む宛先メンバーであることを示すマーカー | ― | Evaluate |

- **標準関数がある capability を宣言した場合、スクリプトに対応する関数を実装する必要があります**。未実装だとスケジューラの呼び出しがエラーになり、そのクリップは描画されません
- 標準関数のない capability は、プロパティやメンバーへの目印として使います（第 5 節の `capability` / `member_capabilities`）
- 上記以外の任意の文字列も書けます。その場合はプラグイン独自の capability（Custom）としてそのまま保持され、標準関数は割り当てられません。capability からコンポーネントを逆引きする仕組み（`PluginLoader::get_components_by_capability`）で利用できます

## 3. フックと実行順序

コンポーネントの標準関数は、フックタイミングの順序で実行されます。順序は `ComponentHook` の宣言順で固定です。

```
Prepare → Prefetch → Evaluate → Process → Finalize
```

現在の実装で標準関数が割り当てられているのは Prefetch（`prefetch`）、Evaluate（`render_frame`、`render_audio`）、Process（`get_transform`、`get_timeline_position`、`get_keyframes`）です。Prepare と Finalize に対応する capability は現時点では存在しません。

描画時の実行は `Scheduler`（`lib/src/plugin/clip/component.rs`）が行います。

1. クリップ種別と同 ID のルートコンポーネントから `children` をたどり、実行対象コンポーネントの一覧に展開する（第 4 節）
2. コンポーネントごとに、そのコンポーネントが宣言したプロパティだけを抽出してスクリプトに渡す
3. capability をフックタイミング順に並べ替え、対応する標準関数を順に呼び出す

> **注意**: 現在の Scheduler は標準関数の戻り値を破棄します。描画結果は、後述する描画コンテキスト（`ctx`）へのメソッド呼び出しという副作用で返してください（[スクリプト API リファレンス](script-api.md)）。

## 4. children の解決

`children` には、ローカル ID または `plugin:local` 形式の名前空間付き ID を書けます。読み込み後の実行時には、ルートコンポーネントから深さ優先でたどってフラットな実行リストに展開されます。

- 同じコンポーネントが複数の枝から参照されても、実行リストには 1 回だけ現れます
- 循環参照がある場合、そのコンポーネント階層の解決はエラーになります
- 存在しないコンポーネントを `children` に書いた場合も解決時にエラーになります

`plugins/std` の例では、`video` コンポーネントが `video-transform` を子として持ちます。`video-transform` 自体はスクリプトを持たず、`transformable` 能力と GUI エディタ（`transform`）の宣言だけを行う定義です。

## 5. プロパティスキーマ

`properties` の各要素はプロパティスキーマで、クリップのプロパティ（GUI で編集され、スクリプトに渡される値）の型と既定値を宣言します。設定項目（`settings.toml`）も同じスキーマ形式を使います。

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `key` | 文字列 | ○ | プロパティのキー。名前空間付き ID は `<plugin_id>:<key>` になります。`:` を含めて階層的なキー（例: `video:media`）にもできます |
| `label` | 文字列 | ○ | GUI に表示するラベル |
| `kind` | テーブル | ○ | 値の型（第 5.1 節） |
| `category` | 文字列の配列 | ― | GUI での分類。省略時は空 |
| `default` | 任意の TOML 値 | ― | 既定値。省略した場合は `kind` から導出されます（第 5.2 節） |
| `capability` | 文字列 | ― | このプロパティが提供する能力。本体はこの宣言を使ってプロパティを逆引きします |
| `member_capabilities` | テーブル | ― | マップ型・グループ型のメンバー名 → capability の対応付け |
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
| `group` | `reference`（必須）, `fields` | 子フィールドをまとめたテーブル | `reference` はグループを参照する名前空間付き ID（`plugin:local` 形式の文字列） |

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

`plugins/std` の video コンポーネントの例:

- プロパティ `video:media`（group）に `capability = "frame-provider"` を宣言し、このプロパティがメディア情報の提供者であることを示す
- そのメンバーには `member_capabilities = { source = "droppable", source_offset = "media-offset-provider" }` を宣言する
- プロパティ `video:transform`（map）には `capability = "transformable"` を宣言する

レンダラー側は、`frame-provider` 能力を持つプロパティからメディア情報を読みます。ソースパスのメンバーキーは `member_capabilities` の `media-source` から解決され、宣言がない場合はメンバー名 `source` にフォールバックします。同様にオフセットは `media-offset-provider` から解決され、なければ `source_offset` にフォールバックします。

グループ型のメンバーは、パス記法で `std:video:media/source` のように表されます（ルートが名前空間付き ID、メンバーが `/` 区切り）。スクリプトの `set_property` などで使います（[スクリプト API リファレンス](script-api.md)）。

## 6. クリップ定義（clips/*.toml）

クリップ種別は `clips/` 配下の TOML に `[[kinds]]` として宣言します。

```toml
[[kinds]]
id = "video"
render_script = "render_clip_video"
tags = ["media"]
```

| フィールド | 型 | 必須 | 説明 |
|------------|------|------|------|
| `id` | 文字列 | ○ | クリップ種別のローカル ID。同 ID のコンポーネントが描画とプロパティを提供します |
| `render_script` | 文字列 | ○ | スクリプト名を保持するフィールド（下記の注意を参照） |
| `tags` | 文字列の配列 | ― | 種別の分類タグ。省略時は空 |

> **注意**: 現在の実装では、描画はコンポーネントの capability に基づくスケジューラ経由で行われ、`render_script` は描画には使用されません。ただし TOML のフィールド自体は必須です。

プロパティはクリップ種別側には書きません。プロパティは各コンポーネントが自身のスキーマとして提供します。
