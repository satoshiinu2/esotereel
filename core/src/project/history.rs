use esotereel_lib::project::Project;
use esotereel_lib::project::command::CommandHistory;
use esotereel_lib::project::ids::TimelineId;

use crate::project::commands::{execute_command, execute_command_undo};

#[derive(Debug)]
pub struct HistoryStack {
    undo_stack: Vec<CommandHistory>,
    redo_stack: Vec<CommandHistory>,
    max_size: usize,
}

impl Default for HistoryStack {
    fn default() -> Self {
        Self::new(100)
    }
}

impl HistoryStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    pub fn push(&mut self, command: CommandHistory) {
        // 新しいコマンドがプッシュされたらredoスタックをクリア
        self.redo_stack.clear();

        // 最大サイズを超える場合は古いコマンドを削除
        if self.undo_stack.len() >= self.max_size {
            self.undo_stack.remove(0);
        }

        self.undo_stack.push(command);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Option<CommandHistory> {
        if let Some(command) = self.undo_stack.pop() {
            self.redo_stack.push(command.clone());
            Some(command)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<CommandHistory> {
        if let Some(command) = self.redo_stack.pop() {
            self.undo_stack.push(command.clone());
            Some(command)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

pub fn execute_command_with_history(
    project: &mut Project,
    timeline_id: TimelineId,
    mut command: CommandHistory,
    history: &mut HistoryStack,
) -> anyhow::Result<()> {
    // コマンドを実行
    crate::project::commands::execute_command(project, timeline_id, &mut command)?;

    // 履歴に追加
    history.push(command);

    Ok(())
}

pub fn undo_command(
    project: &mut Project,
    timeline_id: TimelineId,
    history: &mut HistoryStack,
) -> anyhow::Result<()> {
    if let Some(command) = history.undo_stack.last().cloned() {
        // undo用の逆コマンドを実行
        execute_command_undo(project, timeline_id, command.clone())?;
        history.undo_stack.pop();
        history.redo_stack.push(command);
    }
    Ok(())
}

pub fn redo_command(
    project: &mut Project,
    timeline_id: TimelineId,
    history: &mut HistoryStack,
) -> anyhow::Result<()> {
    if let Some(mut command) = history.redo_stack.last().cloned() {
        // redo用のコマンドを実行
        execute_command(project, timeline_id, &mut command)?;
        history.redo_stack.pop();
        history.undo_stack.push(command);
    }
    Ok(())
}
