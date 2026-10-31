use crate::{
    plugin::{
        NamespacedID,
        property::{
            special::{Media, MediaValues, Transform, TransformValues},
            value::FieldValue,
        },
    },
    project::{TimelineTick, clip::Clip},
};

pub const VIDEO_PLUGIN_ID: &str = "std";
pub const VIDEO_KIND_ID: &str = "video";

pub type VideoMediaProperties = MediaValues;

#[derive(Debug, Clone, PartialEq)]
pub struct VideoClipProperties {
    pub media: VideoMediaProperties,
    pub transform: TransformValues,
}

impl VideoClipProperties {
    pub fn is_video_kind(kind_id: &NamespacedID) -> bool {
        kind_id.plugin_id() == VIDEO_PLUGIN_ID && kind_id.local_id() == VIDEO_KIND_ID
    }

    pub fn from_clip(clip: &Clip) -> anyhow::Result<Self> {
        let property_id = clip.kind_id.clone();
        let binding = clip
            .properties
            .get(&property_id)
            .ok_or_else(|| anyhow::anyhow!("video property group `{property_id}` not found"))?;
        let value = binding
            .in_frame_of()
            .ok_or_else(|| anyhow::anyhow!("video property group `{property_id}` has no value"))?;
        Self::from_value(value)
    }

    pub fn from_value(value: &FieldValue) -> anyhow::Result<Self> {
        let video = as_map(value, "std:video")?;
        let media_reference = Media::reference()?;
        let media = value_at(video.get(media_reference.local_id()).ok_or_else(|| {
            anyhow::anyhow!("video property `{}` not found", media_reference.local_id())
        })?)?;
        let transform_reference = Transform::reference()?;
        let transform = value_at(video.get(transform_reference.local_id()).ok_or_else(|| {
            anyhow::anyhow!(
                "video property `{}` not found",
                transform_reference.local_id()
            )
        })?)?;
        let media = Media::read(media)?;
        let transform = Transform::read(transform)?;

        Ok(Self { media, transform })
    }

    pub fn media_time_seconds(
        &self,
        clip_position: TimelineTick,
        current_frame: TimelineTick,
        ticks_per_second: f64,
    ) -> anyhow::Result<f64> {
        anyhow::ensure!(
            ticks_per_second.is_finite() && ticks_per_second > 0.0,
            "ticks per second must be a positive finite number"
        );
        let relative_frame = current_frame.saturating_sub(clip_position);
        if relative_frame < 0 {
            return Ok(0.0);
        }
        let seconds = (relative_frame as f64 + self.media.source_offset as f64) / ticks_per_second;
        anyhow::ensure!(seconds.is_finite(), "video media time is not finite");
        Ok(seconds)
    }
}

fn as_map<'a>(
    value: &'a FieldValue,
    property: &str,
) -> anyhow::Result<&'a std::collections::BTreeMap<String, FieldValue>> {
    match value {
        FieldValue::Map(values) => Ok(values),
        other => anyhow::bail!("video property `{property}` must be a group, got {other:?}"),
    }
}

fn value_at(value: &FieldValue) -> anyhow::Result<&FieldValue> {
    match value {
        FieldValue::Map(_) => Ok(value),
        other => anyhow::bail!("expected property group, got {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn default_video_value() -> FieldValue {
        FieldValue::Map(
            [
                (
                    "media".into(),
                    FieldValue::Map(
                        [
                            ("source".into(), FieldValue::String("video.mp4".into())),
                            ("source_offset".into(), FieldValue::Int(15)),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                ),
                (
                    "transform".into(),
                    FieldValue::Map(
                        [
                            (
                                "position".into(),
                                FieldValue::Array(vec![
                                    FieldValue::Float(10.0),
                                    FieldValue::Float(20.0),
                                    FieldValue::Float(0.0),
                                ]),
                            ),
                            (
                                "rotation".into(),
                                FieldValue::Array(vec![
                                    FieldValue::Float(0.0),
                                    FieldValue::Float(0.0),
                                    FieldValue::Float(0.0),
                                ]),
                            ),
                            (
                                "scale".into(),
                                FieldValue::Array(vec![
                                    FieldValue::Float(2.0),
                                    FieldValue::Float(0.5),
                                    FieldValue::Float(1.0),
                                ]),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        )
    }

    #[test]
    fn video_properties_apply_source_offset_and_transform() {
        let properties = VideoClipProperties::from_value(&default_video_value()).unwrap();

        assert_eq!(properties.media_time_seconds(100, 145, 30.0).unwrap(), 2.0);
        let matrix = properties.transform.matrix();
        assert_eq!(
            matrix.transform_point3(Vec3::ZERO),
            Vec3::new(10.0, 20.0, 0.0)
        );
        assert_eq!(
            matrix.transform_point3(Vec3::new(1.0, 1.0, 0.0)),
            Vec3::new(12.0, 20.5, 0.0)
        );
    }

    #[test]
    fn video_properties_reject_invalid_transform_vectors() {
        let mut value = default_video_value();
        let FieldValue::Map(video) = &mut value else {
            unreachable!()
        };
        let FieldValue::Map(transform) = video.get_mut("transform").unwrap() else {
            unreachable!()
        };
        transform.insert(
            "scale".into(),
            FieldValue::Array(vec![FieldValue::Float(1.0)]),
        );

        assert!(VideoClipProperties::from_value(&value).is_err());
    }
}
