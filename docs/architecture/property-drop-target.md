# プロパティのファイルドロップ設計

## 概要

ファイルドロップ時、GUI はパスを含む `AddClipFromFile` をサーバーへ送ります。サーバーはクリップ定義から `FileDropHandler` を持つコンポーネントを解決して `handle_file_drop(source_path)` を呼び出し、戻り値のプロパティを検証して初期値に適用します。その後は通常の `AddClip` として処理します。

プロパティの初期値はプラグインが決め、検証・適用とクリップ作成はサーバーが担当します。プラグインは既存プロパティを直接変更しません。

## Handler の契約

Handler はプロパティ ID をキー、プロパティ値を値とする Rhai map を返します。キーはプロパティアドレスの正規形（`std:media#media`）で、Handler 自身のプロパティについては所有者を省略した相対形（`:#media`）でも書けます。値は `FieldValue` に変換可能な値で、グループ／マップの部分指定もできます。指定しなかったプロパティやメンバーはスキーマのデフォルト値を保ちます。

```rhai
fn handle_file_drop(source_path) {
    #{
        "std:media#media": #{
            source: source_path,
        },
    }
}
```

Video コンポーネントの定義では `file-drop-handler` を宣言し、`script` に対応するコンポーネントスクリプトを指定します。標準 Video のソースメンバーは引き続き `media-source` capability で宣言し、レンダラーが値を読むために使います。`media-offset-provider` も編集・描画時に参照する能力であり、ドロップ時の書き込み先指定ではありません。

## サーバー側の検証

`Scheduler::file_drop_properties` は、コンポーネント階層内に Handler がちょうど一つあることを要求します。戻り値に含まれるプロパティ ID はクリップ定義で宣言済みでなければなりません。グループ／マップに未知のメンバーが含まれる場合や、戻り値の型がデフォルト値と合わない場合はエラーにします。空の戻り値も失敗として扱います。

これにより、`Droppable` のような書き込み先専用 capability は不要です。Handler が設定するプロパティと値を返すため、プラグインごとに異なる初期化ロジックを表現できます。

## 現在の制約

- `AddClipFromFile` はファイルパスをサーバーへ送ります。リモートクライアント向けには、参照登録（`RegisterAssetReference`）とアップロード（`BeginAssetUpload` 〜 `FinishAssetUpload`）のリクエストがあり、サーバー側でプロジェクトの `AssetRegistry` へ登録します。`add_clip` を伴う場合は登録後に同じ FileDropHandler でクリップを追加します。GUI からこれらを開始する導線はまだありません。
- `FieldValue::Color` や `FieldValue::Path` などの一部の値形式は、Rhai 戻り値との型変換を今後拡張する必要があります。
