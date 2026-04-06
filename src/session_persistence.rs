use serde::{Deserialize, Serialize};

/// 会话持久化数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub name: String,
    pub tags: Vec<String>,
}

/// 会话列表快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsSnapshot {
    pub version: u32,
    pub sessions: Vec<SessionSnapshot>,
}

impl SessionsSnapshot {
    /// 从会话列表创建快照
    pub fn from_metadata(metadata_list: Vec<(String, Vec<String>)>) -> Self {
        let sessions = metadata_list
            .into_iter()
            .map(|(name, tags)| SessionSnapshot { name, tags })
            .collect();

        SessionsSnapshot {
            version: 1,
            sessions,
        }
    }

    /// 转换为元数据列表
    pub fn to_metadata(self) -> Vec<(String, Vec<String>)> {
        self.sessions
            .into_iter()
            .map(|s| (s.name, s.tags))
            .collect()
    }

    /// 保存到文件
    pub fn save(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        eprintln!("[SessionPersistence] Sessions saved to {}", path.display());
        Ok(())
    }

    /// 从文件加载
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(SessionsSnapshot {
                version: 1,
                sessions: vec![],
            });
        }

        let content = std::fs::read_to_string(path)?;
        let snapshot: SessionsSnapshot = serde_json::from_str(&content)?;
        eprintln!("[SessionPersistence] Sessions loaded from {}", path.display());
        Ok(snapshot)
    }
}

/// 确保会话历史目录存在
pub fn ensure_session_history_dir(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_conversion() {
        let metadata = vec![
            ("Session 1".to_string(), vec!["dev".to_string()]),
            ("Session 2".to_string(), vec!["test".to_string()]),
        ];

        let snapshot = SessionsSnapshot::from_metadata(metadata.clone());
        let restored = snapshot.to_metadata();

        assert_eq!(metadata, restored);
    }
}
