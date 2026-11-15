use super::*;
use crate::plugin::property::value::FieldValue;
use crate::{
    project::{
        clip::{ArchivedClipBindingValue, ClipBindingValue},
        command::{ArchivedCommandRequest, CommandRequest},
    },
    requests::{ArchivedRequest, Request},
};
use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
};

#[test]
fn namespaced_id_has_same_hash_before_and_after_archiving() {
    let id = NamespacedID::new("std", "source").unwrap();
    let bytes = rkyv::to_bytes::<_, 256>(&id).unwrap();
    let archived = rkyv::check_archived_root::<NamespacedID>(&bytes).unwrap();
    let mut original_hash = std::collections::hash_map::DefaultHasher::new();
    let mut archived_hash = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut original_hash);
    archived.hash(&mut archived_hash);
    assert_eq!(original_hash.finish(), archived_hash.finish());
}

#[test]
fn add_clip_properties_survive_network_validation_and_lookup() {
    for count in [1, 2, 7, 32] {
        let properties = (0..count)
            .map(|i| {
                (
                    NamespacedID::new("std", &format!("field_{i}")).unwrap(),
                    ClipBindingValue::Static(FieldValue::Int(i)),
                )
            })
            .collect();
        let request = Request::Command {
            commands: VecDeque::from([(
                1,
                CommandRequest::AddClip {
                    layer_id: 1,
                    position: 0,
                    duration: 60,
                    kind_id: NamespacedID::new("std", "video").unwrap(),
                    properties,
                },
            )]),
        };
        let bytes = rkyv::to_bytes::<_, 1024>(&request).unwrap();
        let archived = rkyv::check_archived_root::<Request>(&bytes).unwrap();
        let ArchivedRequest::Command { commands } = archived else {
            panic!("expected command")
        };
        let ArchivedCommandRequest::AddClip { properties, .. } = &commands[0].1 else {
            panic!("expected AddClip")
        };
        assert_eq!(properties.len(), count as usize);
        for i in 0..count {
            let key = NamespacedID::new("std", &format!("field_{i}")).unwrap();
            let key_bytes = rkyv::to_bytes::<_, 256>(&key).unwrap();
            let archived_key = rkyv::check_archived_root::<NamespacedID>(&key_bytes).unwrap();
            assert!(
                matches!(properties.get(archived_key), Some(ArchivedClipBindingValue::Static(crate::plugin::property::value::ArchivedFieldValue::Int(value))) if *value == i)
            );
        }
        let decoded = rkyv::from_bytes::<Request>(&bytes).unwrap();
        let Request::Command { commands } = decoded else {
            panic!("expected command")
        };
        let CommandRequest::AddClip { properties, .. } = &commands[0].1 else {
            panic!("expected AddClip")
        };
        assert_eq!(properties.len(), count as usize);
    }
}
