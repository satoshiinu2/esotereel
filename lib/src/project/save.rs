use std::path::Path;

use crate::project::model::ProjectModel;

pub fn save_project(project: &ProjectModel, path: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(&project)?;
    std::fs::write(path, json)
}

pub fn load_project(path: &Path) -> std::io::Result<ProjectModel> {
    let json = std::fs::read_to_string(path)?;
    let project: ProjectModel = serde_json::from_str(&json)?;
    Ok(project)
}
