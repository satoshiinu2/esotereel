use super::*;
use esotereel_lib::project::Project;

#[test]
fn build_layer_rows_preserves_insertion_order() {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);

    let five = project
        .insert_layer_in_timeline(timeline_id, None, None, "five".into())
        .unwrap();
    let one = project
        .insert_layer_in_timeline(timeline_id, None, None, "one".into())
        .unwrap();
    let three = project
        .insert_layer_in_timeline(timeline_id, None, None, "three".into())
        .unwrap();

    let mut rows = Vec::new();
    {
        let timeline = project.timeline_ref(timeline_id).unwrap();
        build_layer_rows(
            &project,
            timeline,
            &timeline.outline.roots,
            &HashSet::new(),
            &HashSet::new(),
            0,
            0,
            &mut rows,
        );
    }

    let ids: Vec<u64> = rows
        .iter()
        .filter_map(|row| match row.kind {
            LayerRowKind::Layer(id) => Some(id),
            LayerRowKind::Folder(_) => None,
        })
        .collect();

    // デフォルト4レイヤーの後ろに、insert順のままfive, one, threeが続く
    assert_eq!(&ids[ids.len() - 3..], &[five, one, three]);
}
