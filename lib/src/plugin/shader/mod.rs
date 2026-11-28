//! プラグインが提供するシェーダ定義（`shaders/` ディレクトリ）のモジュールです。
//!
//! シェーダは WGSL ソースとメタデータ TOML の組で 1 つの独立要素として宣言され、
//! コンポーネント等からは名前空間付き ID（例: `std:wave_compute`）で参照されます。
//! 読み込み時に wesl でモジュール解決（`core::common` などの lib 提供モジュールを含む）
//! を行い、naga で検証したコンパイル済み WGSL を定義に保持します。
//!
//! compute については Params パッキングとスタック実行ランナーに加え、
//! クリップのシェーダスタック経由で実レンダーループへ接続済みです
//! （`render::deform` 参照）。render 用シェーダの実行配線は未実装です。

use std::{collections::HashMap, path::Path, sync::Arc};

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
            ShaderProperty::Own(schema) => schema.key.local_name(),
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
/// 頂点定義の単一の定義元で、`lib/shaders/core/common.wesl` に置きます。
/// 頂点レイアウトは `lib/src/render/vertex.rs` の `Vertex`
/// （position vec3 / color vec4 / tex_coords vec2、9 × f32）と一致させます。
/// 一致は `validate_core_library_layout` が読み込み時に検証します。
pub const CORE_COMMON_SOURCE: &str = include_str!("../../../shaders/core/common.wesl");

/// lib がプラグインへ提供するユニフォーム共通モジュールのパスです。
pub const CORE_UNIFORMS_MODULE_PATH: &str = "core::uniforms";

/// lib が提供する共通モジュール（`core::uniforms`）のソースです。
///
/// `ScreenUniform` / `TransformUniform` の単一の定義元で、
/// `lib/shaders/core/uniforms.wesl` に置きます。ホスト側の
/// `render::uniform::ScreenUniform` と変換行列バッファのレイアウトとの
/// 一致は `validate_core_library_layout` が読み込み時に検証します。
pub const CORE_UNIFORMS_SOURCE: &str = include_str!("../../../shaders/core/uniforms.wesl");

/// lib 提供の共通モジュール（モジュールパスとソース）の一覧です。
pub const CORE_MODULES: [(&str, &str); 2] = [
    (CORE_COMMON_MODULE_PATH, CORE_COMMON_SOURCE),
    (CORE_UNIFORMS_MODULE_PATH, CORE_UNIFORMS_SOURCE),
];

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
/// `package_modules` は `(モジュールパス, ソース)` の一覧で、lib 提供の
/// 共通モジュール（`CORE_MODULES`）は自動で登録されます。コンパイルに先立ち、
/// 共通モジュールのレイアウトがホスト側の型と一致するかも検証します
/// （`validate_core_library_layout`）。戻り値は検証済みの WGSL 文字列です。
pub fn compile_and_validate(
    main_module: &str,
    package_modules: &[(String, String)],
) -> anyhow::Result<String> {
    validate_core_library_layout()?;
    compile_modules(main_module, package_modules)
}

fn compile_modules(
    main_module: &str,
    package_modules: &[(String, String)],
) -> anyhow::Result<String> {
    let mut resolver = VirtualResolver::new();
    for (path, source) in CORE_MODULES {
        resolver.add_module(
            path.parse::<ModulePath>()
                .map_err(|e| anyhow::anyhow!("invalid core module path `{path}`: {e}"))?,
            source.into(),
        );
    }
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
    inner: PluginDefinitionRegistry<NamespacedID, Arc<ShaderDefinition>>,
}

impl ShaderRegistry {
    /// 指定 ID の `ShaderDefinition` を返します。存在しなければ `None`。
    pub fn get(&self, id: &NamespacedID) -> Option<Arc<ShaderDefinition>> {
        self.inner.get(id).cloned()
    }

    /// 指定 ID のエントリが存在するかを返します。
    pub fn contains(&self, id: &NamespacedID) -> bool {
        self.inner.contains(id)
    }

    /// 全エントリへのイテレータを返します。
    pub fn iter(&self) -> impl Iterator<Item = (&NamespacedID, &Arc<ShaderDefinition>)> {
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
        definitions: HashMap<NamespacedID, Arc<ShaderDefinition>>,
    ) -> anyhow::Result<()> {
        self.inner.merge(definitions)
    }
}

/// レイアウト検証用のプローブモジュールです。
///
/// 共通モジュールの全型を使うため、wesl の未使用宣言削除で型が消えません。
/// エントリポイント自体は実行されません。
const LAYOUT_PROBE_MODULE_PATH: &str = "package::core_layout_probe";
const LAYOUT_PROBE_SOURCE: &str = "\
import core::common::{ClipVertex, ClipVertexInput, ClipVertexOutput, CLIP_VERTEX_FLOATS};
import core::uniforms::{ScreenUniform, TransformUniform};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;
@group(0) @binding(1)
var<uniform> transform_uniform: TransformUniform;

@vertex
fn vs_main(in: ClipVertexInput) -> ClipVertexOutput {
    var out: ClipVertexOutput;
    out.position = vec4<f32>(in.position, 1.0)
        + transform_uniform.transform * vec4<f32>(0.0)
        + vec4<f32>(screen.size, 0.0, 0.0) * f32(CLIP_VERTEX_FLOATS);
    out.color = in.color;
    out.tex_coords = in.tex_coords;
    var v: ClipVertex;
    v.position = in.position;
    v.color = in.color;
    v.tex_coords = in.tex_coords;
    out.color += vec4<f32>(v.position.x);
    return out;
}
";

/// 共通モジュール（`core::common` / `core::uniforms`）のレイアウトが、ホスト側の
/// 型（`Vertex` / `ScreenUniform` / 変換行列バッファ）と一致するかを検証します。
///
/// 共通モジュールを実際に wesl コンパイルし、naga IR 上の構造体定義・定数を
/// ホスト側のレイアウトと突き合わせます。不一致はエラーとして返し、シェーダの
/// 読み込み自体を失敗させます（黙ってずれたレイアウトで描画しません）。
pub fn validate_core_library_layout() -> anyhow::Result<()> {
    use crate::render::{uniform::ScreenUniform, vertex::Vertex};

    let wgsl = compile_modules(
        LAYOUT_PROBE_MODULE_PATH,
        &[(
            LAYOUT_PROBE_MODULE_PATH.to_string(),
            LAYOUT_PROBE_SOURCE.to_string(),
        )],
    )?;
    let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| {
        anyhow::anyhow!(
            "naga parse failed for core library probe:\n{}",
            e.emit_to_string(&wgsl)
        )
    })?;

    // ClipVertex: フィールドの並びと型が Vertex の f32 詰めと一致すること。
    let clip_members = struct_members(&module, "ClipVertex")?.0;
    let expected_fields = [("position", 3usize), ("color", 4), ("tex_coords", 2)];
    check_vec_fields(&module, "ClipVertex", &clip_members, &expected_fields)?;
    let host_floats = std::mem::size_of::<Vertex>() / std::mem::size_of::<f32>();
    let shader_floats: usize = expected_fields.iter().map(|(_, n)| n).sum();
    anyhow::ensure!(
        shader_floats == host_floats,
        "core::common ClipVertex has {shader_floats} f32 components, host Vertex has {host_floats}"
    );

    // CLIP_VERTEX_FLOATS: 定数値が同じ個数を指すこと。
    let const_value = module
        .constants
        .iter()
        .find(|(_, c)| {
            c.name
                .as_deref()
                .is_some_and(|n| n.ends_with("CLIP_VERTEX_FLOATS"))
        })
        .and_then(|(_, c)| match module.global_expressions.try_get(c.init) {
            Ok(naga::Expression::Literal(naga::Literal::U32(v))) => Some(*v as usize),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("CLIP_VERTEX_FLOATS constant not found in core::common"))?;
    anyhow::ensure!(
        const_value == host_floats,
        "core::common CLIP_VERTEX_FLOATS is {const_value}, host Vertex has {host_floats} floats"
    );

    // ClipVertexInput: @location と型が Vertex::layout() の属性と一致すること。
    let input_members = struct_members(&module, "ClipVertexInput")?.0;
    check_vec_fields(&module, "ClipVertexInput", &input_members, &expected_fields)?;
    let layout = Vertex::layout();
    anyhow::ensure!(
        input_members.len() == layout.attributes.len(),
        "core::common ClipVertexInput has {} fields, host vertex layout has {} attributes",
        input_members.len(),
        layout.attributes.len()
    );
    for (member, attr) in input_members.iter().zip(layout.attributes.iter()) {
        let location = match &member.binding {
            Some(naga::Binding::Location { location, .. }) => *location,
            other => anyhow::bail!(
                "ClipVertexInput field `{}` must have @location, got {other:?}",
                member.name.as_deref().unwrap_or("?")
            ),
        };
        anyhow::ensure!(
            location == attr.shader_location,
            "ClipVertexInput field `{}` is @location({location}), host layout uses {}",
            member.name.as_deref().unwrap_or("?"),
            attr.shader_location
        );
        let components = vec_components(&module, member.ty)
            .ok_or_else(|| anyhow::anyhow!("ClipVertexInput field is not a vec<f32>"))?;
        let format_components = match attr.format {
            wgpu::VertexFormat::Float32 => 1,
            wgpu::VertexFormat::Float32x2 => 2,
            wgpu::VertexFormat::Float32x3 => 3,
            wgpu::VertexFormat::Float32x4 => 4,
            other => anyhow::bail!("unsupported host vertex format {other:?}"),
        };
        anyhow::ensure!(
            components == format_components,
            "ClipVertexInput field `{}` has {components} components, host format {:?} has {format_components}",
            member.name.as_deref().unwrap_or("?"),
            attr.format
        );
    }

    // ScreenUniform: WGSL 側の span / オフセットが #[repr(C)] と一致すること。
    let (screen_members, screen_span) = struct_members(&module, "ScreenUniform")?;
    anyhow::ensure!(
        screen_span as usize == std::mem::size_of::<ScreenUniform>(),
        "core::uniforms ScreenUniform spans {screen_span} bytes, host ScreenUniform is {} bytes",
        std::mem::size_of::<ScreenUniform>()
    );
    let view_projection = screen_members
        .iter()
        .find(|m| m.name.as_deref() == Some("view_projection"))
        .ok_or_else(|| anyhow::anyhow!("ScreenUniform.view_projection not found"))?;
    anyhow::ensure!(
        view_projection.offset as usize == std::mem::offset_of!(ScreenUniform, view_projection),
        "ScreenUniform.view_projection is at offset {} in WGSL, {} on host",
        view_projection.offset,
        std::mem::offset_of!(ScreenUniform, view_projection)
    );
    anyhow::ensure!(
        is_mat4_f32(&module, view_projection.ty),
        "ScreenUniform.view_projection must be mat4x4<f32>"
    );

    // TransformUniform: 変換行列バッファ 1 件分（Mat4）と一致すること。
    let (transform_members, transform_span) = struct_members(&module, "TransformUniform")?;
    anyhow::ensure!(
        transform_span as usize == std::mem::size_of::<glam::Mat4>(),
        "core::uniforms TransformUniform spans {transform_span} bytes, host Mat4 is {} bytes",
        std::mem::size_of::<glam::Mat4>()
    );
    let transform = transform_members
        .first()
        .filter(|m| m.name.as_deref() == Some("transform") && m.offset == 0)
        .ok_or_else(|| anyhow::anyhow!("TransformUniform.transform must be the first field"))?;
    anyhow::ensure!(
        is_mat4_f32(&module, transform.ty),
        "TransformUniform.transform must be mat4x4<f32>"
    );

    Ok(())
}

/// naga IR から、名前が `_{suffix}` で終わる構造体のメンバーと span を返します。
fn struct_members<'m>(
    module: &'m naga::Module,
    suffix: &str,
) -> anyhow::Result<(Vec<naga::StructMember>, u32)> {
    for (_, ty) in module.types.iter() {
        let Some(name) = &ty.name else { continue };
        if !name.ends_with(&format!("_{suffix}")) {
            continue;
        }
        let naga::TypeInner::Struct { members, span } = &ty.inner else {
            continue;
        };
        return Ok((members.clone(), *span));
    }
    anyhow::bail!("struct `{suffix}` not found in compiled core library")
}

/// 構造体メンバーが期待どおりの `vecN<f32>` フィールド列かを検証します。
fn check_vec_fields(
    module: &naga::Module,
    struct_name: &str,
    members: &[naga::StructMember],
    expected: &[(&str, usize)],
) -> anyhow::Result<()> {
    anyhow::ensure!(
        members.len() == expected.len(),
        "{struct_name} has {} fields, expected {}",
        members.len(),
        expected.len()
    );
    for (member, (name, components)) in members.iter().zip(expected.iter()) {
        anyhow::ensure!(
            member.name.as_deref() == Some(*name),
            "{struct_name} field is `{}`, expected `{name}`",
            member.name.as_deref().unwrap_or("?")
        );
        let actual = vec_components(module, member.ty)
            .ok_or_else(|| anyhow::anyhow!("{struct_name}.{name} is not a vec<f32>"))?;
        anyhow::ensure!(
            actual == *components,
            "{struct_name}.{name} is vec{actual}<f32>, expected vec{components}<f32>"
        );
    }
    Ok(())
}

/// 型が `vecN<f32>` なら成分数を返します。
fn vec_components(module: &naga::Module, ty: naga::Handle<naga::Type>) -> Option<usize> {
    let naga::TypeInner::Vector { size, scalar } = module.types[ty].inner else {
        return None;
    };
    if scalar.kind != naga::ScalarKind::Float || scalar.width != 4 {
        return None;
    }
    Some(match size {
        naga::VectorSize::Bi => 2,
        naga::VectorSize::Tri => 3,
        naga::VectorSize::Quad => 4,
    })
}

/// 型が `mat4x4<f32>` かを返します。
fn is_mat4_f32(module: &naga::Module, ty: naga::Handle<naga::Type>) -> bool {
    matches!(
        module.types[ty].inner,
        naga::TypeInner::Matrix {
            columns: naga::VectorSize::Quad,
            rows: naga::VectorSize::Quad,
            scalar,
        } if scalar.kind == naga::ScalarKind::Float && scalar.width == 4
    )
}
