//! プラグインが提供するシェーダ定義（`shaders/` ディレクトリ）のモジュールです。
//!
//! シェーダは WGSL ソースとメタデータ TOML の組で 1 つの独立要素として宣言され、
//! コンポーネント等からは名前空間付き ID（例: `std:wave_compute`）で参照されます。
//! 読み込み時に wesl でモジュール解決（`core::common` などの lib 提供モジュールを含む）
//! を行い、naga で検証したコンパイル済み WGSL を定義に保持します。
//!
//! compute については Params パッキングとスタック実行ランナーの基盤がありますが、
//! 実レンダーループへの接続は未実装です。render 用シェーダの実行配線も未実装です。

use std::{collections::HashMap, path::Path};

use anyhow::Context;
use wesl::{Compiler, resolver::VirtualResolver, syntax::ModulePath};

use crate::plugin::{NamespacedID, property::PropertySchema, registry::PluginDefinitionRegistry};

pub mod compute;
pub mod params;
pub mod parse;
pub use compute::{ComputePass, ComputeStackRunner};
pub use params::{CoreParamValue, pack_params};
pub use parse::parse_shader_definition;

/// シェーダの種別とエントリポイントです。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderKind {
    /// 描画用シェーダ（頂点＋フラグメント）。
    Render {
        entry_vertex: String,
        entry_fragment: String,
    },
    /// compute 用シェーダ（頂点変形など）。
    Compute { entry_compute: String },
}

/// lib が値を供給する共通パラメータ（`core:` プレフィックス）の WGSL 型です。
///
/// 型は lib 側で固定され、プラグインは TOML で `kind` / `default` を指定できません。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreParamType {
    Float32,
    Vec2F32,
    Mat4x4F32,
}

impl CoreParamType {
    /// WGSL 上の型名を返します。
    pub fn wgsl_type(&self) -> &'static str {
        match self {
            CoreParamType::Float32 => "f32",
            CoreParamType::Vec2F32 => "vec2<f32>",
            CoreParamType::Mat4x4F32 => "mat4x4<f32>",
        }
    }
}

/// 既知の `core:` パラメータキーの型を返します。未知のキーは `None` です。
pub fn core_param_type(key: &str) -> Option<CoreParamType> {
    match key {
        "core:time" => Some(CoreParamType::Float32),
        "core:screen_size" => Some(CoreParamType::Vec2F32),
        "core:view_projection" | "core:transform" => Some(CoreParamType::Mat4x4F32),
        _ => None,
    }
}

/// シェーダ定義のプロパティ 1 件です。宣言順がそのまま Params uniform の詰め順になります。
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderProperty {
    /// lib が値を供給する共通パラメータ。キーは `core:` プレフィックス付きです。
    Core {
        key: String,
        value_type: CoreParamType,
    },
    /// シェーダ固有のプロパティ。スキーマはコンポーネントのプロパティと共通です。
    Own(PropertySchema),
}

impl ShaderProperty {
    /// 宣言されたキー文字列を返します（`core:` 項目はそのまま、固有項目はローカル ID）。
    pub fn key_str(&self) -> &str {
        match self {
            ShaderProperty::Core { key, .. } => key,
            ShaderProperty::Own(schema) => schema.key.local_id(),
        }
    }
}

/// 読み込まれたシェーダ定義です。
#[derive(Debug, Clone)]
pub struct ShaderDefinition {
    /// 名前空間付き ID（例: `std:wave_compute`）。
    pub id: NamespacedID,
    /// 表示名。
    pub name: String,
    /// 種別とエントリポイント。
    pub kind: ShaderKind,
    /// `shaders/` からの相対ソースファイル名（TOML の `source`）。
    pub source: String,
    /// 宣言順のプロパティ一覧。
    pub properties: Vec<ShaderProperty>,
    /// wesl 解決・naga 検証済みの WGSL。読み込み時に設定されます。
    pub compiled_wgsl: String,
}

/// lib がプラグインへ提供する共通モジュールのパスです。
pub const CORE_COMMON_MODULE_PATH: &str = "core::common";

/// lib が提供する共通モジュール（`core::common`）のソースです。
///
/// 頂点レイアウトは `lib/src/render/vertex.rs` の `Vertex`
/// （position vec3 / color vec4 / tex_coords vec2、9 × f32）と一致させます。
pub const CORE_COMMON_SOURCE: &str = "\
// core::common — provided by Esotereel lib, not by plugins.
// Cross-package imports require `public` (no keyword = package-visible only).
public struct ClipVertex {
    position: vec3<f32>,
    color: vec4<f32>,
    tex_coords: vec2<f32>,
}

public const CLIP_VERTEX_FLOATS: u32 = 9u;
";

/// `shaders/` からの相対パスを wesl のモジュールパスへ変換します。
///
/// 例: `wave.wgsl` → `package::wave`、`effects/wave.wgsl` → `package::effects::wave`
pub fn module_path_for(relative: &Path) -> anyhow::Result<String> {
    let stem = relative.with_extension("");
    let mut parts: Vec<&str> = stem
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .with_context(|| format!("invalid shader module path `{}`", relative.display()))?;
    parts.insert(0, "package");
    Ok(parts.join("::"))
}

/// プラグインのモジュール群から `main_module` を wesl でコンパイルし、naga で検証します。
///
/// `package_modules` は `(モジュールパス, ソース)` の一覧で、`core::common` は
/// lib 提供分が自動で登録されます。戻り値は検証済みの WGSL 文字列です。
pub fn compile_and_validate(
    main_module: &str,
    package_modules: &[(String, String)],
) -> anyhow::Result<String> {
    let mut resolver = VirtualResolver::new();
    resolver.add_module(
        CORE_COMMON_MODULE_PATH
            .parse::<ModulePath>()
            .map_err(|e| anyhow::anyhow!("invalid core module path: {e}"))?,
        CORE_COMMON_SOURCE.into(),
    );
    for (path, source) in package_modules {
        let module_path = path
            .parse::<ModulePath>()
            .map_err(|e| anyhow::anyhow!("invalid shader module path `{path}`: {e}"))?;
        resolver.add_module(module_path, source.clone().into());
    }

    let main_path = main_module
        .parse::<ModulePath>()
        .map_err(|e| anyhow::anyhow!("invalid shader module path `{main_module}`: {e}"))?;
    let compiler = Compiler::default().with_resolver(resolver);
    let compiled = compiler
        .compile_module(&main_path)
        .map_err(|e| anyhow::anyhow!("wesl compile failed for `{main_module}`: {e}"))?;
    let wgsl = compiled.to_string();

    let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| {
        anyhow::anyhow!(
            "naga parse failed for `{main_module}`:\n{}",
            e.emit_to_string(&wgsl)
        )
    })?;
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .map_err(|e| anyhow::anyhow!("naga validation failed for `{main_module}`: {e:?}"))?;

    Ok(wgsl)
}

/// 複数プラグインから合成したシェーダ定義の一覧です。
/// `ComponentRegistry` と対になる位置づけで、ID 衝突チェック付きでマージします。
#[derive(Debug, Default)]
pub struct ShaderRegistry {
    inner: PluginDefinitionRegistry<NamespacedID, ShaderDefinition>,
}

impl ShaderRegistry {
    /// 指定 ID の `ShaderDefinition` への参照を返します。存在しなければ `None`。
    pub fn get(&self, id: &NamespacedID) -> Option<&ShaderDefinition> {
        self.inner.get(id)
    }

    /// 指定 ID のエントリが存在するかを返します。
    pub fn contains(&self, id: &NamespacedID) -> bool {
        self.inner.contains(id)
    }

    /// 全エントリへのイテレータを返します。
    pub fn iter(&self) -> impl Iterator<Item = (&NamespacedID, &ShaderDefinition)> {
        self.inner.iter()
    }

    /// 登録件数を返します。
    pub fn len(&self) -> usize {
        self.inner.iter().count()
    }

    /// 空かどうかを返します。
    pub fn is_empty(&self) -> bool {
        self.inner.iter().count() == 0
    }

    /// 既存定義と id 衝突があればエラーにして合流させる。
    pub fn merge_shader_definitions(
        &mut self,
        definitions: HashMap<NamespacedID, ShaderDefinition>,
    ) -> anyhow::Result<()> {
        self.inner.merge(definitions)
    }
}
