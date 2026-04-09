use serde::{Deserialize, Serialize};

/// 会话持久化数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub name: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

/// 会话列表快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsSnapshot {
    pub version: u32,
    pub sessions: Vec<SessionSnapshot>,
}

impl SessionsSnapshot {
    /// 从会话快照列表创建
    pub fn from_snapshots(sessions: Vec<SessionSnapshot>) -> Self {
        SessionsSnapshot {
            version: 1,
            sessions,
        }
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
        let snapshots = vec![
            SessionSnapshot { name: "Session 1".to_string(), tags: vec!["dev".to_string()], cwd: Some("/home/user".to_string()) },
            SessionSnapshot { name: "Session 2".to_string(), tags: vec!["test".to_string()], cwd: Some("/tmp".to_string()) },
        ];

        let snapshot = SessionsSnapshot::from_snapshots(snapshots);
        assert_eq!(snapshot.sessions.len(), 2);
        assert_eq!(snapshot.sessions[0].cwd, Some("/home/user".to_string()));
        assert_eq!(snapshot.sessions[1].cwd, Some("/tmp".to_string()));
    }
}
