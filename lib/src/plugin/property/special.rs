use std::collections::BTreeMap;

use glam::{EulerRot, Mat4, Quat, Vec3};
use rhai::{Dynamic, Map};

use crate::{
    plugin::{
        NamespacedID,
        property::{PropertyPath, PropertySchema, value::FieldTypeKind, value::FieldValue},
    },
    project::TimelineTick,
};

pub struct Media;

impl Media {
    pub fn reference() -> anyhow::Result<NamespacedID> {
        NamespacedID::new("core", "media")
    }

    pub fn label() -> &'static str {
        "Media"
    }

    pub fn fields() -> anyhow::Result<Vec<PropertySchema>> {
        Ok(vec![Source::schema()?, SourceOffset::schema()?])
    }

    pub fn read(value: &FieldValue) -> anyhow::Result<MediaValues> {
        let fields = as_map(value, Self::label())?;
        Ok(MediaValues {
            source: Source::read(fields)?,
            source_offset: SourceOffset::read(fields)?,
        })
    }

    pub fn source_offset_path(root: &NamespacedID) -> anyhow::Result<PropertyPath> {
        let reference = Self::reference()?;
        PropertyPath::from(root.clone())
            .child(reference.local_id())?
            .child(SourceOffset::key())
    }
}

pub struct Source;

impl Source {
    fn key() -> &'static str {
        "source"
    }

    fn schema() -> anyhow::Result<PropertySchema> {
        field(
            Self::key(),
            "Source",
            FieldTypeKind::String,
            FieldValue::String(String::new()),
        )
    }

    fn read(fields: &BTreeMap<String, FieldValue>) -> anyhow::Result<String> {
        match fields.get(Self::key()) {
            Some(FieldValue::String(source)) => Ok(source.clone()),
            Some(value) => anyhow::bail!(
                "media property `{}` must be a string, got {value:?}",
                Self::key()
            ),
            None => anyhow::bail!("media property `{}` not found", Self::key()),
        }
    }
}

pub struct SourceOffset;

impl SourceOffset {
    fn key() -> &'static str {
        "source_offset"
    }

    fn schema() -> anyhow::Result<PropertySchema> {
        field(
            Self::key(),
            "Source Offset",
            FieldTypeKind::Int {
                min: None,
                max: None,
            },
            FieldValue::Int(TimelineTick::default()),
        )
    }

    fn read(fields: &BTreeMap<String, FieldValue>) -> anyhow::Result<TimelineTick> {
        match fields.get(Self::key()) {
            Some(FieldValue::Int(offset)) => Ok(*offset),
            Some(value) => anyhow::bail!(
                "media property `{}` must be an integer, got {value:?}",
                Self::key()
            ),
            None => anyhow::bail!("media property `{}` not found", Self::key()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaValues {
    pub source: String,
    pub source_offset: TimelineTick,
}

pub struct Transform;

impl Transform {
    pub fn reference() -> anyhow::Result<NamespacedID> {
        NamespacedID::new("core", "transform")
    }

    pub fn label() -> &'static str {
        "Transform"
    }

    pub fn fields() -> anyhow::Result<Vec<PropertySchema>> {
        Ok(vec![
            Position::schema()?,
            Rotation::schema()?,
            Scale::schema()?,
        ])
    }

    pub fn read(value: &FieldValue) -> anyhow::Result<TransformValues> {
        let fields = as_map(value, Self::label())?;
        Ok(TransformValues {
            position: Position::read(fields)?,
            rotation: Rotation::read(fields)?,
            scale: Scale::read(fields)?,
        })
    }

    pub fn from_rhai_map(fields: &Map) -> Result<TransformValues, Box<rhai::EvalAltResult>> {
        Ok(TransformValues {
            position: read_rhai_vector(fields, Position::key())?,
            rotation: read_rhai_vector(fields, Rotation::key())?,
            scale: read_rhai_vector(fields, Scale::key())?,
        })
    }
}

pub struct Position;

impl Position {
    fn key() -> &'static str {
        "position"
    }

    fn schema() -> anyhow::Result<PropertySchema> {
        vector_field(Self::key(), "Position", 1.0, [0.0, 0.0, 0.0])
    }

    fn read(fields: &BTreeMap<String, FieldValue>) -> anyhow::Result<[f32; 3]> {
        read_vector(fields, Self::key())
    }
}

pub struct Rotation;

impl Rotation {
    fn key() -> &'static str {
        "rotation"
    }

    fn schema() -> anyhow::Result<PropertySchema> {
        vector_field(Self::key(), "Rotation", 1.0, [0.0, 0.0, 0.0])
    }

    fn read(fields: &BTreeMap<String, FieldValue>) -> anyhow::Result<[f32; 3]> {
        read_vector(fields, Self::key())
    }
}

pub struct Scale;

impl Scale {
    fn key() -> &'static str {
        "scale"
    }

    fn schema() -> anyhow::Result<PropertySchema> {
        vector_field(Self::key(), "Scale", 1.0, [100.0, 100.0, 1.0])
    }

    fn read(fields: &BTreeMap<String, FieldValue>) -> anyhow::Result<[f32; 3]> {
        read_vector(fields, Self::key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformValues {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl TransformValues {
    pub fn matrix(self) -> Mat4 {
        let rotation = Vec3::from_array(self.rotation) * (std::f32::consts::PI / 180.0);
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z),
            Vec3::from_array(self.position),
        )
    }
}

pub fn resolve(reference: &NamespacedID) -> anyhow::Result<Option<(String, Vec<PropertySchema>)>> {
    if reference == &Media::reference()? {
        return Ok(Some((Media::label().to_owned(), Media::fields()?)));
    }
    if reference == &Transform::reference()? {
        return Ok(Some((Transform::label().to_owned(), Transform::fields()?)));
    }
    Ok(None)
}

fn vector_field(
    key: &str,
    label: &str,
    step: f64,
    default: [f64; 3],
) -> anyhow::Result<PropertySchema> {
    field(
        key,
        label,
        FieldTypeKind::Array {
            item_kind: Box::new(FieldTypeKind::Float {
                min: None,
                max: None,
                step,
            }),
        },
        FieldValue::Array(default.into_iter().map(FieldValue::Float).collect()),
    )
}

fn field(
    key: &str,
    label: &str,
    kind: FieldTypeKind,
    default: FieldValue,
) -> anyhow::Result<PropertySchema> {
    Ok(PropertySchema {
        key: NamespacedID::new("core", key)?,
        category: Vec::new(),
        label: label.to_owned(),
        kind,
        default,
    })
}

fn as_map<'a>(
    value: &'a FieldValue,
    property: &str,
) -> anyhow::Result<&'a BTreeMap<String, FieldValue>> {
    match value {
        FieldValue::Map(fields) => Ok(fields),
        other => anyhow::bail!("special property `{property}` must be a group, got {other:?}"),
    }
}

fn read_vector(fields: &BTreeMap<String, FieldValue>, key: &str) -> anyhow::Result<[f32; 3]> {
    let values = match fields.get(key) {
        Some(FieldValue::Array(values)) => values,
        Some(value) => anyhow::bail!("transform property `{key}` must be an array, got {value:?}"),
        None => anyhow::bail!("transform property `{key}` not found"),
    };
    anyhow::ensure!(
        values.len() == 3,
        "transform property `{key}` must contain exactly three numbers"
    );

    let mut vector = [0.0; 3];
    for (output, value) in vector.iter_mut().zip(values) {
        let number = match value {
            FieldValue::Float(value) => *value,
            FieldValue::Int(value) => *value as f64,
            other => {
                anyhow::bail!("transform property `{key}` must contain numbers, got {other:?}")
            }
        };
        *output = number as f32;
        anyhow::ensure!(
            number.is_finite() && output.is_finite(),
            "transform property `{key}` must contain finite numbers"
        );
    }
    Ok(vector)
}

fn read_rhai_vector(fields: &Map, key: &str) -> Result<[f32; 3], Box<rhai::EvalAltResult>> {
    let values = fields
        .get(key)
        .cloned()
        .and_then(Dynamic::try_cast::<rhai::Array>)
        .ok_or_else(|| {
            Box::<rhai::EvalAltResult>::from(format!("transform property `{key}` must be an array"))
        })?;
    if values.len() != 3 {
        return Err(
            format!("transform property `{key}` must contain exactly three numbers").into(),
        );
    }

    let mut vector = [0.0; 3];
    for (output, value) in vector.iter_mut().zip(values) {
        let number = value
            .clone()
            .try_cast::<f64>()
            .or_else(|| value.try_cast::<i64>().map(|value| value as f64))
            .ok_or_else(|| {
                Box::<rhai::EvalAltResult>::from(format!(
                    "transform property `{key}` must contain numbers"
                ))
            })?;
        *output = number as f32;
        if !number.is_finite() || !output.is_finite() {
            return Err(format!("transform property `{key}` must contain finite numbers").into());
        }
    }
    Ok(vector)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_scale_defaults_to_temporary_preview_size() {
        assert_eq!(
            Scale::schema().unwrap().default,
            FieldValue::Array(vec![
                FieldValue::Float(100.0),
                FieldValue::Float(100.0),
                FieldValue::Float(1.0),
            ])
        );
    }
}
