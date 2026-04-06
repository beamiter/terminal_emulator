use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// 命令补全候选项
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub description: String,
    pub insert_text: String,
}

/// 补全类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CompletionKind {
    Command,
    File,
    Directory,
    History,
    Variable,
}

impl ToString for CompletionKind {
    fn to_string(&self) -> String {
        match self {
            CompletionKind::Command => "Command",
            CompletionKind::File => "File",
            CompletionKind::Directory => "Directory",
            CompletionKind::History => "History",
            CompletionKind::Variable => "Variable",
        }
        .to_string()
    }
}

/// Shell 历史管理
#[derive(Clone, Debug, Default)]
pub struct CommandHistory {
    commands: VecDeque<String>,
    max_size: usize,
}

impl CommandHistory {
    pub fn new(max_size: usize) -> Self {
        CommandHistory {
            commands: VecDeque::new(),
            max_size,
        }
    }

    pub fn add(&mut self, command: String) {
        if !command.is_empty() && (self.commands.is_empty() || self.commands[0] != command) {
            self.commands.push_front(command);
            if self.commands.len() > self.max_size {
                self.commands.pop_back();
            }
        }
    }

    pub fn get_history(&self) -> Vec<String> {
        self.commands.iter().cloned().collect()
    }

    pub fn search(&self, pattern: &str) -> Vec<String> {
        self.commands
            .iter()
            .filter(|cmd| cmd.contains(pattern))
            .cloned()
            .collect()
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(&self.commands)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let commands: VecDeque<String> = serde_json::from_str(&content)?;
            Ok(CommandHistory {
                commands,
                max_size: 1000,
            })
        } else {
            Ok(CommandHistory::new(1000))
        }
    }
}

/// 命令补全引擎
pub struct CompletionEngine {
    history: CommandHistory,
    system_commands_cache: Option<Vec<String>>,
}

impl CompletionEngine {
    pub fn new() -> Self {
        CompletionEngine {
            history: CommandHistory::new(1000),
            system_commands_cache: None,
        }
    }

    /// 获取系统中的可执行命令（缓存）
    pub fn get_system_commands(&mut self) -> Vec<String> {
        if let Some(ref cache) = self.system_commands_cache {
            return cache.clone();
        }

        let mut commands = Vec::new();

        // 从 PATH 环境变量获取所有可执行文件
        if let Ok(path_env) = std::env::var("PATH") {
            for path_str in path_env.split(':') {
                if let Ok(entries) = std::fs::read_dir(path_str) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            commands.push(name.to_string());
                        }
                    }
                }
            }
        }

        // 排序和去重
        commands.sort();
        commands.dedup();

        // 缓存结果
        self.system_commands_cache = Some(commands.clone());
        commands
    }

    /// 补全命令
    pub fn complete_command(&mut self, prefix: &str, limit: usize) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // 从历史中查找
        for cmd in self.history.search(prefix).iter().take(limit / 2) {
            items.push(CompletionItem {
                label: cmd.clone(),
                kind: CompletionKind::History,
                description: "From history".to_string(),
                insert_text: cmd.clone(),
            });
        }

        // 从系统命令中查找
        let system_cmds = self.get_system_commands();
        for cmd in system_cmds.iter().filter(|c| c.starts_with(prefix)).take(limit / 2) {
            items.push(CompletionItem {
                label: cmd.clone(),
                kind: CompletionKind::Command,
                description: "System command".to_string(),
                insert_text: cmd.clone(),
            });
        }

        // 限制结果数量
        items.truncate(limit);
        items
    }

    /// 补全文件路径
    pub fn complete_file(&self, path_prefix: &str, limit: usize) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        let path = PathBuf::from(path_prefix);
        let parent = if path_prefix.ends_with('/') {
            path.clone()
        } else {
            path.parent().unwrap_or_else(|| std::path::Path::new("/")).to_path_buf()
        };

        if let Ok(entries) = std::fs::read_dir(&parent) {
            for entry in entries.flatten().take(limit) {
                if let Some(name) = entry.file_name().to_str() {
                    let is_dir = entry.file_type().ok().map(|ft| ft.is_dir()).unwrap_or(false);
                    let full_name = if is_dir {
                        format!("{}/", name)
                    } else {
                        name.to_string()
                    };

                    items.push(CompletionItem {
                        label: full_name.clone(),
                        kind: if is_dir {
                            CompletionKind::Directory
                        } else {
                            CompletionKind::File
                        },
                        description: "File/Directory".to_string(),
                        insert_text: full_name,
                    });
                }
            }
        }

        items
    }

    /// 添加命令到历史
    pub fn add_to_history(&mut self, command: String) {
        self.history.add(command);
    }

    /// 获取补全提示（基于命令前缀）
    pub fn get_parameter_hints(command: &str) -> Option<String> {
        let hints = HashMap::from([
            ("ls", "-la (list all files with details)"),
            ("grep", "-r (search recursively)"),
            ("find", "-name (search by name)"),
            ("sed", "-i (edit in place)"),
            ("awk", "-F (set field separator)"),
        ]);

        hints.get(command).map(|s| s.to_string())
    }
}

impl Default for CompletionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_history() {
        let mut history = CommandHistory::new(10);
        history.add("ls -la".to_string());
        history.add("cd /tmp".to_string());

        assert_eq!(history.get_history().len(), 2);
    }

    #[test]
    fn test_history_search() {
        let mut history = CommandHistory::new(10);
        history.add("ls -la".to_string());
        history.add("ls -l".to_string());

        let results = history.search("ls");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parameter_hints() {
        let hint = CompletionEngine::get_parameter_hints("grep");
        assert!(hint.is_some());
    }
}
