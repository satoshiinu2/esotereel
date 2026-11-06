use rkyv::{Archive, CheckBytes, bytecheck};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
    plugin::{
        NamespacedID,
        property::{PropertyPath, PropertySchema, value::FieldValue},
    },
    project::{
        TimelineTick,
        ids::{ClipId, TimelineId},
    },
};

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Clip {
    pub id: ClipId,
    pub position: TimelineTick,
    pub duration: TimelineTick,

    pub kind_id: NamespacedID,
    pub properties: HashMap<NamespacedID, ClipBindingValue>,
}

impl Clip {
    pub fn new(
        id: ClipId,
        position: TimelineTick,
        duration: TimelineTick,
        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,
    ) -> Self {
        Self {
            id,
            position,
            duration,
            kind_id,
            properties,
        }
    }

    pub fn position(&self) -> i64 {
        self.position
    }

    pub fn set_position(&mut self, new_pos: i64) {
        self.position = new_pos;
    }

    pub fn get_property_value(&self, key: &NamespacedID) -> Option<&ClipBindingValue> {
        self.properties.get(key)
    }

    pub fn get_property_path(&self, path: &PropertyPath) -> Option<&FieldValue> {
        let mut value = self.properties.get(path.root())?.in_frame_of()?;
        for member in path.members() {
            let FieldValue::Map(values) = value else {
                return None;
            };
            value = values.get(member)?;
        }
        Some(value)
    }

    pub fn set_property_path(
        &mut self,
        path: &PropertyPath,
        value: ClipBindingValue,
    ) -> anyhow::Result<()> {
        if path.members().is_empty() {
            return self.set_property_value(path.root().clone(), value);
        }
        let target = self
            .properties
            .get_mut(path.root())
            .ok_or_else(|| anyhow::anyhow!("property group `{}` not found", path.root()))?;
        let ClipBindingValue::Static(target_value) = target;
        let mut current = target_value;
        for member in path.members() {
            let FieldValue::Map(values) = current else {
                anyhow::bail!("property path `{path}` traverses a non-group");
            };
            current = values
                .get_mut(member)
                .ok_or_else(|| anyhow::anyhow!("property path `{path}` not found"))?;
        }
        let ClipBindingValue::Static(value) = value;
        *current = value;
        Ok(())
    }

    pub fn set_property_value(
        &mut self,
        key: NamespacedID,
        value: ClipBindingValue,
    ) -> anyhow::Result<()> {
        self.properties.insert(key, value);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ClipKind {
    pub render_script: String,
    pub tags: HashSet<String>,
}

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub enum ClipBindingValue {
    Static(FieldValue),
    // 将来的に Keyframes(Vec<(TimelineTick, FieldValue)>) 等を追加予定
}

impl ClipBindingValue {
    // TODO: 将来的に Keyframes等を追加したら、ここもそれに応じて変更する
    pub fn in_frame_of(&self) -> Option<&FieldValue> {
        match self {
            ClipBindingValue::Static(v) => Some(v),
        }
    }

    pub fn default_properties(
        schema: &[PropertySchema],
    ) -> HashMap<NamespacedID, ClipBindingValue> {
        schema
            .iter()
            .map(|field| {
                (
                    field.key.clone(),
                    ClipBindingValue::Static(field.default.clone()),
                )
            })
            .collect()
    }
}

/// Compositionへの参照。Mirrorは複数Clipが同じTimelineIdを共有し、
/// Independentはこのclip専用のprivate Timelineを指す。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[archive_attr(derive(CheckBytes))]
pub enum CompositionRef {
    Mirror(TimelineId),
    Independent(TimelineId),
}

impl CompositionRef {
    pub fn timeline_id(&self) -> TimelineId {
        match self {
            CompositionRef::Mirror(id) | CompositionRef::Independent(id) => *id,
        }
    }
}

/// Scriptに渡すパラメータ。今はプレースホルダー。
/// rhai等のスクリプトエンジンと繋ぐ際に型を差し替える想定。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Default,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ScriptParams(pub BTreeMap<String, String>);

impl PartialEq for Clip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Clip {}

impl PartialEq for ArchivedClip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for ArchivedClip {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::property::value::FieldTypeKind;

    #[test]
    fn clip_position_get_set() {
        let clip: Clip = Clip::new(
            1u64,
            0,
            60,
            NamespacedID::new("std", "video").unwrap(),
            HashMap::new(),
        );
        assert_eq!(clip.position(), 0);

        let mut clip = clip;
        clip.set_position(120);
        assert_eq!(clip.position(), 120);
    }

    #[test]
    fn clip_set_and_get_property_value() {
        let mut clip = Clip::new(
            1u64,
            0,
            60,
            NamespacedID::new("std", "video").unwrap(),
            HashMap::new(),
        );
        clip.set_property_value(
            NamespacedID::new("std", "source").unwrap(),
            ClipBindingValue::Static(FieldValue::String("clip.mp4".to_string())),
        )
        .unwrap();
        assert!(matches!(
            clip.get_property_value(&NamespacedID::new("std", "source").unwrap()),
            Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "clip.mp4"
        ));
    }

    #[test]
    fn clip_default_properties_from_schema() {
        let schema = vec![PropertySchema {
            key: NamespacedID::new("std", "volume").unwrap(),
            category: vec![],
            label: "Volume".to_string(),
            kind: FieldTypeKind::Float {
                min: Some(0.0),
                max: Some(1.0),
                step: 0.1,
            },
            default: FieldValue::Float(0.5),
            capability: None,
            member_capabilities: BTreeMap::new(),
        }];
        let defaults = ClipBindingValue::default_properties(&schema);
        assert_eq!(defaults.len(), 1);
        assert!(matches!(
            defaults.get(&NamespacedID::new("std", "volume").unwrap()),
            Some(ClipBindingValue::Static(FieldValue::Float(v))) if *v == 0.5
        ));
    }

    #[test]
    fn clip_get_set_property_path() {
        let mut clip = Clip::new(
            1u64,
            0,
            60,
            NamespacedID::new("std", "video").unwrap(),
            ClipBindingValue::default_properties(&[
                PropertySchema::group(
                    NamespacedID::new("std", "transform").unwrap(),
                    "Transform".to_string(),
                    NamespacedID::new("std", "transform-ref").unwrap(),
                    vec![PropertySchema {
                        key: NamespacedID::new("std", "x").unwrap(),
                        category: vec![],
                        label: "X".to_string(),
                        kind: FieldTypeKind::Float {
                            min: None,
                            max: None,
                            step: 1.0,
                        },
                        default: FieldValue::Float(10.0),
                        capability: None,
                        member_capabilities: BTreeMap::new(),
                    }],
                ),
            ]),
        );
        let path = PropertyPath::parse("std:transform/x").unwrap();
        assert_eq!(
            clip.get_property_path(&path),
            Some(&FieldValue::Float(10.0))
        );

        clip.set_property_path(
            &path,
            ClipBindingValue::Static(FieldValue::Float(99.0)),
        )
        .unwrap();
        assert_eq!(
            clip.get_property_path(&path),
            Some(&FieldValue::Float(99.0))
        );
    }
}
