//! Mixin 宣言の型と、解決済みのフック表を提供します。
//!
//! Mixin は、他のプラグインが提供する関数（標準関数や lib の API）の呼び出しに、
//! 別プラグインが宣言的に処理を差し込むための仕組みです。プラグインは
//! `mixins/` ディレクトリに TOML で宣言（`[[mixins]]`）を置き、`script`
//! フィールドで関連する Rhai ハンドラを指定します。
//!
//! ## 実装状況
//! 現在は読み込みフェーズのみが実装されています。宣言のパース・検証と、
//! 対象関数ごとのフック表（`MixinIndex`）の構築までを行います。
//! ハンドラスクリプトのコンパイルと実行時 dispatch は未実装であり、
//! 宣言があっても実行時の挙動は変わりません。

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::plugin::{property::ComponentCapability, NamespacedID};

pub mod parse;
pub use parse::parse_mixin_declarations;

/// フックを差し込む位置（対象関数の呼び出しの前／後）。
///
/// `BTreeMap` のキーやソートに使えるよう `Ord` も実装している
/// （`ComponentHook` の「宣言順 = 実行順」の流儀に倣う）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MixinAt {
    Before,
    After,
}

/// 優先度が省略された場合のデフォルト値（叩き台の仮置き）。
pub const DEFAULT_PRIORITY: i64 = 1000;

/// 1 件の mixin 宣言（パース・検証・名前解決済み）。
///
/// 対象は `target_fn`（関数名）で指定し、`target_component` /
/// `target_capability` を併せて指定すると対象を絞り込めます。
/// 絞り込みの解決（どの宣言がどの呼び出しに適用されるか）は dispatch
/// 側の責務であり、この型は宣言そのものの保持に徹します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixinDeclaration {
    /// 宣言の名前空間付き ID（`plugin_id:local_id`）。全プラグインを通じて一意。
    pub id: NamespacedID,
    /// フック対象の関数名（例: `render_frame`）。
    pub target_fn: String,
    /// 対象を特定のコンポーネントに絞る場合のコンポーネント ID。
    #[serde(default)]
    pub target_component: Option<NamespacedID>,
    /// 対象を capability で絞る場合の能力。未知の文字列はパース時にエラーになる。
    #[serde(default)]
    pub target_capability: Option<ComponentCapability>,
    /// 注入位置（呼び出しの前／後）。
    pub at: MixinAt,
    /// 実行優先度（大きい方が先）。省略時は `DEFAULT_PRIORITY`。
    #[serde(default)]
    pub priority: Option<i64>,
    /// ハンドラスクリプトのパス（`mixins/` ディレクトリからの相対パス）。
    pub script: String,
    /// ハンドラ関数名。
    pub handler: String,
}

impl MixinDeclaration {
    /// 優先度の実効値を返します（省略時はデフォルト値）。
    pub fn effective_priority(&self) -> i64 {
        self.priority.unwrap_or(DEFAULT_PRIORITY)
    }
}

/// 解決済みのフック表です。
///
/// 対象関数名をキーに宣言を保持します。各関数に対する宣言リストは
/// 優先度（降順）→ 宣言 ID（昇順）の順で決定的に並びます。
/// 宣言 ID は全プラグインを通じて一意であることを `build` で検証するため、
/// この並びはプラグインの読み込み順に依存しません。
#[derive(Debug, Clone, Default)]
pub struct MixinIndex {
    by_fn: BTreeMap<String, Vec<MixinDeclaration>>,
}

impl MixinIndex {
    /// 全プラグイン分の宣言からフック表を構築します。
    ///
    /// 宣言 ID が重複した場合はエラーにします（`rebuild_indices` が
    /// コンポーネント定義の ID 衝突をエラーにする流儀に倣う）。
    pub fn build(
        declarations: impl IntoIterator<Item = MixinDeclaration>,
    ) -> anyhow::Result<Self> {
        let mut seen: HashSet<NamespacedID> = HashSet::new();
        let mut by_fn: BTreeMap<String, Vec<MixinDeclaration>> = BTreeMap::new();
        for declaration in declarations {
            if !seen.insert(declaration.id.clone()) {
                anyhow::bail!(
                    "duplicate mixin declaration id `{}` across plugins",
                    declaration.id
                );
            }
            by_fn
                .entry(declaration.target_fn.clone())
                .or_default()
                .push(declaration);
        }
        for declarations in by_fn.values_mut() {
            declarations.sort_by(|a, b| {
                b.effective_priority()
                    .cmp(&a.effective_priority())
                    .then_with(|| a.id.full().cmp(b.id.full()))
            });
        }
        Ok(Self { by_fn })
    }

    /// 指定した関数名への宣言を、優先度順で返します。
    pub fn get(&self, target_fn: &str) -> &[MixinDeclaration] {
        self.by_fn.get(target_fn).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 宣言の総数を返します。
    pub fn len(&self) -> usize {
        self.by_fn.values().map(Vec::len).sum()
    }

    /// 宣言が 1 件も無いかどうかを返します。
    pub fn is_empty(&self) -> bool {
        self.by_fn.is_empty()
    }

    /// 対象関数名と宣言リストのペアを、関数名順に列挙します。
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<MixinDeclaration>)> {
        self.by_fn.iter()
    }
}
