use crate::session::Session;
use crate::terminal::TerminalState;
use crate::shell::ShellSession;
use std::sync::Arc;
use parking_lot::Mutex as ParkingMutex;

/// 获取当前工作目录
fn get_current_working_dir() -> Option<String> {
    // 尝试读取当前进程的 cwd
    std::fs::read_link("/proc/self/cwd")
        .ok()
        .and_then(|path| path.to_str().map(|s| s.to_string()))
        .or_else(|| {
            // fallback: 使用标准库的 current_dir
            std::env::current_dir()
                .ok()
                .and_then(|path| path.to_str().map(|s| s.to_string()))
        })
}

/// SessionManager - 管理所有终端会话
pub struct SessionManager {
    sessions: Vec<Session>,
    active_index: usize,
}

impl SessionManager {
    /// 创建新的会话管理器，初始化一个默认会话
    pub fn new(first_session: Session) -> Self {
        SessionManager {
            sessions: vec![first_session],
            active_index: 0,
        }
    }

    /// 创建新会话并添加到管理器，继承当前工作目录
    pub fn new_session(&mut self, name: Option<String>, tags: Option<Vec<String>>) -> usize {
        let index = self.sessions.len();
        let name = name.unwrap_or_else(|| format!("Session {}", index + 1));
        let tags = tags.unwrap_or_default();

        // 获取当前活跃会话的工作目录（如果存在）
        let cwd = if !self.sessions.is_empty() {
            get_current_working_dir()
        } else {
            None
        };

        // 创建新会话，继承工作目录
        let cwd_ref = cwd.as_deref();
        match ShellSession::new_with_cwd(80, 24, cwd_ref) {
            Ok(shell) => {
                let terminal = Arc::new(ParkingMutex::new(TerminalState::new(80, 24)));
                let session = Session::new(name, tags, terminal, shell);
                self.sessions.push(session);
                index
            }
            Err(e) => {
                eprintln!("Failed to create new session: {}", e);
                index
            }
        }
    }

    /// 关闭指定会话
    pub fn close_session(&mut self, index: usize) -> bool {
        if index >= self.sessions.len() {
            return false;
        }

        if self.sessions.len() == 1 {
            // 不允许关闭最后一个会话
            return false;
        }

        self.sessions.remove(index);

        // 调整活跃会话索引
        if self.active_index >= self.sessions.len() {
            self.active_index = self.sessions.len() - 1;
        }

        true
    }

    /// 切换到指定会话
    pub fn switch_session(&mut self, index: usize) -> bool {
        if index < self.sessions.len() {
            self.active_index = index;
            if let Some(session) = self.sessions.get_mut(index) {
                session.metadata.update_last_active();
            }
            true
        } else {
            false
        }
    }

    /// 切换到下一个会话
    pub fn switch_to_next_session(&mut self) -> usize {
        self.active_index = (self.active_index + 1) % self.sessions.len();
        self.active_index
    }

    /// 切换到前一个会话
    pub fn switch_to_prev_session(&mut self) -> usize {
        if self.active_index == 0 {
            self.active_index = self.sessions.len() - 1;
        } else {
            self.active_index -= 1;
        }
        self.active_index
    }

    /// 获取当前活跃会话的索引
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// 获取当前活跃会话（可变引用）
    pub fn get_active_session_mut(&mut self) -> &mut Session {
        &mut self.sessions[self.active_index]
    }

    /// 获取当前活跃会话（不可变引用）
    pub fn get_active_session(&self) -> &Session {
        &self.sessions[self.active_index]
    }

    /// 获取指定索引的会话（可变引用）
    pub fn get_session_mut(&mut self, index: usize) -> Option<&mut Session> {
        self.sessions.get_mut(index)
    }

    /// 获取指定索引的会话（不可变引用）
    pub fn get_session(&self, index: usize) -> Option<&Session> {
        self.sessions.get(index)
    }

    /// 获取所有会话的不可变引用
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// 获取所有会话的可变引用
    pub fn sessions_mut(&mut self) -> &mut [Session] {
        &mut self.sessions
    }

    /// 会话总数
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// 向指定会话添加标签
    pub fn add_tag(&mut self, session_index: usize, tag: String) {
        if let Some(session) = self.sessions.get_mut(session_index) {
            session.add_tag(tag);
        }
    }

    /// 从指定会话删除标签
    pub fn remove_tag(&mut self, session_index: usize, tag: &str) {
        if let Some(session) = self.sessions.get_mut(session_index) {
            session.remove_tag(tag);
        }
    }

    /// 获取包含指定标签的所有会话索引
    pub fn get_sessions_by_tag(&self, tag: &str) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| session.has_tag(tag))
            .map(|(index, _)| index)
            .collect()
    }

    /// 获取所有唯一的标签
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags = std::collections::HashSet::new();
        for session in &self.sessions {
            for tag in &session.metadata.tags {
                tags.insert(tag.clone());
            }
        }
        let mut result: Vec<_> = tags.into_iter().collect();
        result.sort();
        result
    }

    /// 重命名会话
    pub fn rename_session(&mut self, session_index: usize, name: String) {
        if let Some(session) = self.sessions.get_mut(session_index) {
            session.metadata.name = name;
            session.metadata.update_last_active();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意: 完整的单元测试需要创建真实的 TerminalState 和 ShellSession
    // 这里只测试基本逻辑
}
