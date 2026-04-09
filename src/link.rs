/// 链接检测和交互模块
use regex::Regex;
use std::path::Path;

/// 链接类型
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum LinkType {
    /// URL (http/https/ftp)
    Url,
    /// 本地文件路径
    FilePath,
    /// IP 地址
    IpAddress,
}

/// 单个链接的信息
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// 链接所在行
    pub line: usize,
    /// 列起始位置
    pub col_start: usize,
    /// 列结束位置（不含）
    pub col_end: usize,
    /// 链接类型
    pub link_type: LinkType,
    /// 链接文本/URL
    pub text: String,
}

/// 链接检测配置
#[derive(Clone, Debug)]
pub struct LinkDetectionConfig {
    /// 是否检测 URL
    pub detect_urls: bool,
    /// 是否检测文件路径
    pub detect_file_paths: bool,
    /// 是否检测 IP 地址
    pub detect_ip_addresses: bool,
}

impl Default for LinkDetectionConfig {
    fn default() -> Self {
        Self {
            detect_urls: true,
            detect_file_paths: true,
            detect_ip_addresses: true,
        }
    }
}

/// 链接检测引擎
pub struct LinkDetector {
    config: LinkDetectionConfig,
    url_regex: Regex,
    ip_regex: Regex,
    file_path_regex: Regex,
}

impl LinkDetector {
    pub fn new(config: LinkDetectionConfig) -> Self {
        // URL 正则：http(s)?:// 或 ftp://
        let url_regex = Regex::new(
            r"(?:https?|ftp)://[^\s<>\[\]{}|\\^`()]*[^\s<>\[\]{}|\\^`().,;:!?\-]"
        ).unwrap();

        // IP 地址正则：x.x.x.x 格式
        let ip_regex = Regex::new(
            r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b"
        ).unwrap();

        // 文件路径正则：以 / 开头或 ./ ../，或包含路径分隔符的文本
        let file_path_regex = Regex::new(
            r"(?:^|[\s])(?:(?:/[^\s<>\[\]{}|\\^`()]*)|(?:\./[^\s<>\[\]{}|\\^`()]*)|(?:\.\./[^\s<>\[\]{}|\\^`()]*))"
        ).unwrap();

        Self {
            config,
            url_regex,
            ip_regex,
            file_path_regex,
        }
    }

    /// 将字节偏移转换为字符（列）偏移
    fn byte_offset_to_char_offset(line: &str, byte_offset: usize) -> usize {
        line[..byte_offset].chars().count()
    }

    /// 在单行文本中检测所有链接
    pub fn detect_links_in_line(&self, line: &str, line_idx: usize) -> Vec<Link> {
        let mut links = Vec::new();

        // 检测 URL
        if self.config.detect_urls {
            for mat in self.url_regex.find_iter(line) {
                let col_start = Self::byte_offset_to_char_offset(line, mat.start());
                let col_end = Self::byte_offset_to_char_offset(line, mat.end());
                links.push(Link {
                    line: line_idx,
                    col_start,
                    col_end,
                    link_type: LinkType::Url,
                    text: mat.as_str().to_string(),
                });
            }
        }

        // 检测 IP 地址
        if self.config.detect_ip_addresses {
            for mat in self.ip_regex.find_iter(line) {
                let col_start = Self::byte_offset_to_char_offset(line, mat.start());
                let col_end = Self::byte_offset_to_char_offset(line, mat.end());
                // 避免与 URL 重复
                if !links.iter().any(|l| {
                    l.col_start <= col_start && col_end <= l.col_end
                }) {
                    links.push(Link {
                        line: line_idx,
                        col_start,
                        col_end,
                        link_type: LinkType::IpAddress,
                        text: mat.as_str().to_string(),
                    });
                }
            }
        }

        // 检测文件路径
        if self.config.detect_file_paths {
            for mat in self.file_path_regex.find_iter(line) {
                let matched_text = mat.as_str().trim();
                let col_start = Self::byte_offset_to_char_offset(line, mat.start());
                let col_end = Self::byte_offset_to_char_offset(line, mat.end());

                // 避免与 URL 重复
                if !links.iter().any(|l| {
                    l.col_start <= col_start && col_end <= l.col_end
                }) && Self::is_valid_file_path(matched_text) {
                    links.push(Link {
                        line: line_idx,
                        col_start,
                        col_end,
                        link_type: LinkType::FilePath,
                        text: matched_text.to_string(),
                    });
                }
            }
        }

        links
    }

    /// 判断文本是否为有效的文件路径
    fn is_valid_file_path(text: &str) -> bool {
        let trimmed = text.trim();

        // 必须以 / 或 ./ 或 ../ 开头
        trimmed.starts_with('/')
            || trimmed.starts_with("./")
            || trimmed.starts_with("../")
            || (trimmed.len() > 0 && trimmed.chars().next().unwrap().is_alphabetic())
    }

    /// 在整个网格中检测链接（带缓存）
    pub fn detect_all_links(&self, grid: &crate::terminal::TerminalGrid) -> Vec<Link> {
        let mut all_links = Vec::new();

        for (line_idx, line) in grid.iter().enumerate() {
            let line_str: String = line.iter().map(|cell| cell.character).collect();
            let links = self.detect_links_in_line(&line_str, line_idx);
            all_links.extend(links);
        }

        all_links
    }
}

/// 打开链接
pub fn open_link(link: &Link) -> Result<(), Box<dyn std::error::Error>> {
    match link.link_type {
        LinkType::Url => {
            open_url(&link.text)?;
        }
        LinkType::FilePath => {
            open_file_path(&link.text)?;
        }
        LinkType::IpAddress => {
            // IP 地址可以用浏览器打开或显示 whois 信息
            open_url(&format!("http://{}", &link.text))?;
        }
    }
    Ok(())
}

/// 打开 URL（使用系统默认浏览器）
#[cfg(target_os = "linux")]
fn open_url(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_url(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_url(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::process::Command::new("cmd")
        .args(&["/C", "start", url])
        .spawn()?;
    Ok(())
}

/// 打开文件路径（使用系统默认应用）
#[cfg(target_os = "linux")]
fn open_file_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let expanded_path = expand_path(path);

    // 如果是目录，用文件管理器打开；否则用默认应用打开
    if Path::new(&expanded_path).is_dir() {
        std::process::Command::new("xdg-open")
            .arg(&expanded_path)
            .spawn()?;
    } else {
        std::process::Command::new("xdg-open")
            .arg(&expanded_path)
            .spawn()?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_file_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let expanded_path = expand_path(path);
    std::process::Command::new("open")
        .arg(&expanded_path)
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_file_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let expanded_path = expand_path(path);
    std::process::Command::new("explorer")
        .arg(&expanded_path)
        .spawn()?;
    Ok(())
}

/// 扩展路径（~/ 变量替换等）
fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!(
                "{}{}",
                home.display(),
                &path[1..]
            );
        }
    }
    path.to_string()
}

/// 复制链接到剪贴板
pub fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("xclip")
            .args(&["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_detection() {
        let detector = LinkDetector::new(LinkDetectionConfig::default());
        let line = "Visit https://example.com for more info";
        let links = detector.detect_links_in_line(line, 0);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, LinkType::Url);
        assert_eq!(links[0].text, "https://example.com");
    }

    #[test]
    fn test_ip_detection() {
        let detector = LinkDetector::new(LinkDetectionConfig::default());
        let line = "Server at 192.168.1.1 is down";
        let links = detector.detect_links_in_line(line, 0);

        assert!(links.iter().any(|l| l.link_type == LinkType::IpAddress));
    }

    #[test]
    fn test_file_path_detection() {
        let detector = LinkDetector::new(LinkDetectionConfig::default());
        let line = "Check /etc/hosts file";
        let links = detector.detect_links_in_line(line, 0);

        assert!(links.iter().any(|l| l.link_type == LinkType::FilePath));
    }

    #[test]
    fn test_link_detection_config() {
        let mut config = LinkDetectionConfig::default();
        config.detect_urls = false;

        let detector = LinkDetector::new(config);
        let line = "Visit https://example.com for more info";
        let links = detector.detect_links_in_line(line, 0);

        assert!(!links.iter().any(|l| l.link_type == LinkType::Url));
    }
}
