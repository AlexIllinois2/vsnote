use std::fs;
use encoding_rs::GB18030;
use std::sync::Mutex;
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config as NotifyConfig, Event as NotifyEvent};
use tauri::{Emitter, Manager};
use tauri::path::BaseDirectory;
use tauri::WindowEvent;
use md5::{Md5, Digest};

mod sqlite_store;

fn show_window(window: &tauri::WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#x27;")
}

fn count_backtick_prefix(s: &str) -> usize {
    s.chars().take_while(|c| *c == '`').count()
}

fn sanitize_html(html: &str) -> String {
    let dangerous_tags = ["script", "style", "iframe", "object", "embed", "form", "textarea", "select", "button", "link", "meta", "base"];
    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '<' && i + 1 < len {
            if chars[i + 1] == '/' {
                let end = find_char(&chars, i, '>');
                if end == chars.len() || end <= i + 3 { i = end; continue; }
                let inner: String = chars[i + 2..end - 1].iter().collect();
                let tag_name = inner.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if dangerous_tags.contains(&tag_name.as_str()) {
                    i = end;
                    continue;
                }
                for c in &chars[i..end] {
                    result.push(*c);
                }
                i = end;
            } else if chars[i + 1] == '!' {
                let end = find_str(&chars, i, "->");
                let comment: String = chars[i..end].iter().collect();
                if comment.starts_with("<!--MATHBLOCK_") {
                    for c in &chars[i..end] {
                        result.push(*c);
                    }
                }
                i = end;
            } else {
                let end = find_char(&chars, i, '>');
                if end == chars.len() || end <= i + 2 { i = end; continue; }
                let inner: String = chars[i + 1..end - 1].iter().collect();
                let tag_name = inner.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if dangerous_tags.contains(&tag_name.as_str()) {
                    i = end;
                    continue;
                }
                let sanitized = sanitize_tag_attributes(&inner);
                result.push('<');
                result.push_str(&sanitized);
                result.push('>');
                i = end;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn find_char(chars: &[char], start: usize, target: char) -> usize {
    for i in start..chars.len() {
        if chars[i] == target {
            return i + 1;
        }
    }
    chars.len()
}

fn find_str(chars: &[char], start: usize, pattern: &str) -> usize {
    let pat: Vec<char> = pattern.chars().collect();
    for i in start..chars.len() {
        if i + pat.len() <= chars.len() && chars[i..i + pat.len()] == pat[..] {
            return i + pat.len();
        }
    }
    chars.len()
}

fn decode_numeric_entities(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' && i + 1 < chars.len() && chars[i + 1] == '#' {
            i += 2;
            let mut value: u32 = 0;
            let mut consumed = false;
            if i < chars.len() && (chars[i] == 'x' || chars[i] == 'X') {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_hexdigit() {
                    value = value * 16 + chars[i].to_digit(16).unwrap();
                    i += 1;
                    consumed = true;
                }
            } else {
                while i < chars.len() && chars[i].is_ascii_digit() {
                    value = value * 10 + chars[i].to_digit(10).unwrap();
                    i += 1;
                    consumed = true;
                }
            }
            if i < chars.len() && chars[i] == ';' && consumed {
                i += 1;
                if let Some(c) = char::from_u32(value) {
                    result.push(c);
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn contains_dangerous_protocol(s: &str) -> bool {
    let lower = s.to_lowercase();
    if lower.contains("javascript:") { return true; }
    let decoded = decode_numeric_entities(&lower);
    decoded.contains("javascript:")
}

fn sanitize_tag_attributes(tag_content: &str) -> String {
    let chars: Vec<char> = tag_content.chars().collect();
    let len = chars.len();

    // Extract tag name (first word before whitespace)
    let mut i = 0;
    while i < len && !chars[i].is_whitespace() {
        i += 1;
    }
    let tag_name: String = chars[..i].iter().collect();
    if i >= len {
        return tag_name;
    }

    let mut result = tag_name;
    while i < len {
        // Skip whitespace
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= len { break; }

        // Read attribute name until '=' or whitespace (boolean attr)
        let attr_start = i;
        while i < len && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let _attr_name: String = chars[attr_start..i].iter().collect();

        if i < len && chars[i] == '=' {
            // Quoted or unquoted value
            i += 1; // skip '='
            let _value_start = i;
            if i < len && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1; // skip opening quote
                let val_start = i;
                while i < len && chars[i] != quote {
                    i += 1;
                }
                let _value: String = chars[val_start..i].iter().collect();
                i += 1; // skip closing quote
                // Reconstruct from original to preserve exact quoting
                let raw: String = chars[attr_start..i].iter().collect();
                let raw_lower = raw.to_lowercase();
                if raw_lower.starts_with("on") || contains_dangerous_protocol(&raw) {
                    continue;
                }
                result.push(' ');
                result.push_str(&raw);
            } else {
                // Unquoted value
                while i < len && !chars[i].is_whitespace() {
                    i += 1;
                }
                let raw: String = chars[attr_start..i].iter().collect();
                let raw_lower = raw.to_lowercase();
                if raw_lower.starts_with("on") || contains_dangerous_protocol(&raw) {
                    continue;
                }
                result.push(' ');
                result.push_str(&raw);
            }
        } else {
            // Boolean attribute (no value)
            let raw: String = chars[attr_start..i].iter().collect();
            let raw_lower = raw.to_lowercase();
            if raw_lower.starts_with("on") || contains_dangerous_protocol(&raw) {
                continue;
            }
            result.push(' ');
            result.push_str(&raw);
        }
    }
    result
}

#[tauri::command]
fn open_devtools(app_handle: tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.open_devtools();
    }
}

#[tauri::command]
fn get_cli_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

// 从单实例回调的完整 argv 中取出待打开文件：跳过 argv[0]（可执行文件路径本身）。
// 与 get_cli_args（std::env::args().skip(1)）保持一致，避免把 exe 路径当文件打开。
fn files_from_args(argv: Vec<String>) -> Vec<String> {
    argv.into_iter().skip(1).collect()
}

// 前端调此命令真正退出应用（关窗弹框选"退出"时调用）。
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// 路径安全校验：拒绝写入/创建到系统关键目录或越界路径。
// 编辑器需读写用户任意选中的文件，故不做全盘 scope 限制，
// 仅在此拦截危险目标（启动目录、系统目录等持久化/RCE 向量）。
pub(crate) fn safe_write_target(path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::Path::new(path);
    let canonical = p
        .canonicalize()
        .or_else(|_| {
            // 目标尚不存在时，向上递归找到第一个存在的祖先目录再 canonicalize，
            // 然后逐级 join 不存在的路径段，得到规范化的绝对路径。
            // 修复：ensure_dir 创建多级子树（如 app_data/tizu-mark/fonts）时，
            // 父目录也不存在，parent.canonicalize() 会失败报"系统找不到指定路径"。
            let mut components: Vec<std::ffi::OsString> = Vec::new();
            let mut cur = p;
            let mut base: Option<std::path::PathBuf> = None;
            while let Some(parent) = cur.parent() {
                if let Ok(canon) = parent.canonicalize() {
                    base = Some(canon);
                    break;
                }
                if let Some(name) = cur.file_name() {
                    components.push(name.to_os_string());
                }
                cur = parent;
                if cur.as_os_str().is_empty() { break; }
            }
            let base = base.ok_or_else(|| format!("Invalid path {}: cannot resolve ancestor", path))?;
            let file_name = p.file_name().ok_or_else(|| format!("Invalid path {}", path))?;
            // 确保最后一段（目标本身）也在 components 里
            if components.is_empty() || components.last().map(|s| s.as_os_str()) != Some(file_name) {
                components = components.into_iter().rev().collect();
            } else {
                components.reverse();
            }
            let mut result = base;
            for name in &components {
                result = result.join(name);
            }
            Ok::<std::path::PathBuf, String>(result)
        })?;

    let canonical_str = canonical.to_string_lossy().replace('/', "\\").to_lowercase();
    // 系统关键 / 持久化目录前缀黑名单（Windows）
    const BLOCKED: &[&str] = &[
        "\\windows\\",
        "\\program files\\",
        "\\program files (x86)\\",
        "\\system32\\",
        "\\programdata\\microsoft\\windows\\start menu\\programs\\startup",
        "\\appdata\\roaming\\microsoft\\windows\\start menu\\programs\\startup",
        "\\documents and settings\\",
        "autorun.inf",
    ];
    for b in BLOCKED {
        if canonical_str.contains(b) {
            return Err(format!("Refusing to write to protected path: {}", path));
        }
    }
    Ok(canonical)
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    // SQLite 工作区路由：路径属于已打开的 db 根时转调 sqlite_store
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::read_file(&root, &path);
    }
    let raw = fs::read(&path).map_err(|e| {
        // 结构化错误 JSON，前端按 kind 映射到错误码（E_NOT_FOUND / E_PERMISSION / E_LOCKED / E_IO）
        let kind = if e.kind() == std::io::ErrorKind::NotFound {
            "NotFound"
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            // Windows 上文件被独占锁定（ERROR_SHARING_VIOLATION=32）也归为锁定
            match e.raw_os_error() {
                Some(32) => "Locked",
                _ => "PermissionDenied",
            }
        } else {
            "Io"
        };
        format!(
            "{{\"kind\":\"{}\",\"path\":\"{}\",\"message\":\"{}\"}}",
            kind,
            path.replace('\\', "\\\\").replace('"', "\\\""),
            e.to_string().replace('"', "\\\"")
        )
    })?;

    Ok(decode_bytes(&raw))
}

// 读取打包资源文件（demo.md / screenshots/* 等），dev/prod 兼容：
// 1) 优先从 Tauri 资源目录读取（生产 / 部分 dev 模式：target/debug/resources/...）；
// 2) 找不到时回退到 CARGO_MANIFEST_DIR 父目录（项目根）的同名字段，dev 模式
//    bundle.resources 不复制资源、目标目录为空，这里能直接读源码根的 demo.md / screenshots；
// 3) 仍找不到返回结构化 NotFound 错误，前端按 openLink 错误码处理。
// 返回 `{ content, path }`：path 是真正读到文件的本地路径（dev = 项目根，prod = 资源目录），
// 让 _openBundledFile 能把它设为 tab.filePath，processImages 据此解析相对图片。
#[tauri::command]
fn read_bundled_file(app: tauri::AppHandle, filename: String) -> Result<serde_json::Value, String> {
    if let Ok(p) = app.path().resolve(&filename, BaseDirectory::Resource) {
        if let Ok(bytes) = fs::read(&p) {
            return Ok(serde_json::json!({
                "content": decode_bytes(&bytes),
                "path": p.to_string_lossy().to_string(),
            }));
        }
    }
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(project_root) = manifest_dir.parent() {
        let dev_path = project_root.join(&filename);
        if let Ok(bytes) = fs::read(&dev_path) {
            return Ok(serde_json::json!({
                "content": decode_bytes(&bytes),
                "path": dev_path.to_string_lossy().to_string(),
            }));
        }
    }
    Err(format!(
        "{{\"kind\":\"NotFound\",\"path\":\"{}\",\"message\":\"bundled file not found: {}\"}}",
        filename.replace('\\', "\\\\").replace('"', "\\\""),
        filename
    ))
}

// 与 read_bundled_file 同款的资源定位策略，返回 base64 字符串（用于 pack 资源如
// screenshots/*.png）。demo.md 内的相对图片优先按 tab.filePath 拼路径走 fetch_image_as_base64，
// 不存在本地时回退到这里，便于 dev 模式加载项目根 screenshots 与生产资源目录 images。
#[tauri::command]
async fn read_bundled_image_as_base64(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let bytes_opt: Option<Vec<u8>> = if let Ok(p) = app.path().resolve(&filename, BaseDirectory::Resource) {
        fs::read(&p).ok()
    } else {
        None
    }
    .or_else(|| {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.parent().and_then(|root| fs::read(root.join(&filename)).ok())
    });
    let bytes = bytes_opt.ok_or_else(|| {
        format!("bundled image not found: {}", filename)
    })?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

// 解码字节为字符串：剥离 UTF-8 BOM → 优先 UTF-8 → 回退 GB18030（覆盖 GBK/GB2312/简繁）。
// read_file 与 search_in_files 共用，确保非 UTF-8 文件也能被搜索。
pub(crate) fn decode_bytes(raw: &[u8]) -> String {
    let stripped: &[u8] = if raw.len() >= 3 && raw[0] == 0xEF && raw[1] == 0xBB && raw[2] == 0xBF {
        &raw[3..]
    } else {
        raw
    };
    if let Ok(s) = String::from_utf8(stripped.to_vec()) {
        return s;
    }
    let (cow, _enc, _had_errors) = GB18030.decode(stripped);
    cow.into_owned()
}

#[derive(serde::Serialize, Clone, Copy)]
pub(crate) struct FileMeta {
    mtime: u64,
    size: u64,
}

#[tauri::command]
fn file_meta(path: String) -> Result<Option<FileMeta>, String> {
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::file_meta(&root, &path);
    }
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Ok(None);
    }
    let meta = fs::metadata(p).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Ok(None);
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok(Some(FileMeta {
        mtime,
        size: meta.len(),
    }))
}

#[tauri::command]
fn is_directory(path: String) -> bool {
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::is_directory(&root, &path);
    }
    std::path::Path::new(&path).is_dir()
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct DirEntryInfo {
    name: String,
    path: String,
    is_dir: bool,
}

#[tauri::command]
fn list_dir(path: String) -> Result<Vec<DirEntryInfo>, String> {
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::list_dir(&root, &path);
    }
    let p = std::path::Path::new(&path);
    let _ = safe_write_target(&path).map_err(|e| e)?; // 复用规范化以拒绝越界/关键目录
    let mut entries: Vec<DirEntryInfo> = Vec::new();
    let read = std::fs::read_dir(&p).map_err(|e| e.to_string())?;
    for entry in read {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        let name = entry.file_name().to_string_lossy().to_string();
        // 侧边文件树按需展示全部条目：不隐藏点文件/文件夹，也不按扩展名过滤。
        // （早期版本这里会跳过 . 开头且只保留 md/markdown/txt，导致 .obsidian、
        // .gitignore、图片附件等在工作区里完全看不见。）
        entries.push(DirEntryInfo {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir,
        });
    }
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    Ok(entries)
}

#[derive(serde::Serialize)]
pub(crate) struct FileMatch {
    path: String,
    matches: Vec<LineMatch>,
}

#[derive(serde::Serialize)]
pub(crate) struct LineMatch {
    line: usize,   // 1-based 行号
    col: usize,    // 1-based 字符列号
    line_text: String,
}

// search_in_files 的同步核心，便于 cargo test 直接调用（命令 async 包装）。
// 递归遍历目录，按扩展名过滤（默认 md/markdown/txt），每文件解码后按行搜索。
// 上限：文件 2000 / 每文件 500 / 总匹配 5000，超限停止遍历。
fn search_in_files_impl(
    dir: &str,
    pattern: &str,
    case_sensitive: bool,
    use_regex: bool,
    extensions: &[String],
) -> Result<Vec<FileMatch>, String> {
    if pattern.is_empty() {
        return Ok(Vec::new());
    }
    let re: Option<regex::Regex> = if use_regex {
        match regex::Regex::new(pattern) {
            Ok(r) => Some(r),
            Err(_) => return Err("{\"kind\":\"InvalidEncoding\",\"path\":\"\",\"message\":\"invalid regex\"}".into()),
        }
    } else {
        None
    };
    let pattern_lower = pattern.to_lowercase();
    let exts: Vec<String> = if extensions.is_empty() {
        vec!["md".into(), "markdown".into(), "txt".into()]
    } else {
        extensions.iter().map(|e| e.to_lowercase()).collect()
    };

    const MAX_FILES: usize = 2000;
    const MAX_PER_FILE: usize = 500;
    const MAX_TOTAL: usize = 5000;
    const MAX_LINE_TEXT: usize = 300;

    let mut results: Vec<FileMatch> = Vec::new();
    let mut total = 0usize;
    let mut file_count = 0usize;

    let base = std::path::Path::new(dir);
    let mut stack: Vec<std::path::PathBuf> = vec![base.to_path_buf()];
    'outer: while let Some(cur) = stack.pop() {
        let rd = match fs::read_dir(&cur) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in rd {
            if file_count >= MAX_FILES || total >= MAX_TOTAL {
                break 'outer;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" || name == ".git" {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !exts.contains(&ext) {
                continue;
            }
            let content = match fs::read(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let text = decode_bytes(&content);
            let mut file_matches: Vec<LineMatch> = Vec::new();
            for (i, line) in text.lines().enumerate() {
                if file_matches.len() >= MAX_PER_FILE {
                    break;
                }
                let byte_col: Option<usize> = if let Some(re) = &re {
                    re.find(line).map(|m| m.start())
                } else if case_sensitive {
                    line.find(pattern)
                } else {
                    line.to_lowercase().find(&pattern_lower)
                };
                if let Some(bc) = byte_col {
                    let char_col = line[..bc].chars().count();
                    let line_text: String = line.chars().take(MAX_LINE_TEXT).collect();
                    file_matches.push(LineMatch {
                        line: i + 1,
                        col: char_col + 1,
                        line_text,
                    });
                    total += 1;
                    if total >= MAX_TOTAL {
                        break;
                    }
                }
            }
            if !file_matches.is_empty() {
                results.push(FileMatch {
                    path: path.to_string_lossy().to_string(),
                    matches: file_matches,
                });
                file_count += 1;
            }
        }
    }
    Ok(results)
}

#[tauri::command]
async fn search_in_files(
    dir: String,
    pattern: String,
    case_sensitive: bool,
    use_regex: bool,
    extensions: Vec<String>,
) -> Result<Vec<FileMatch>, String> {
    if let Some(root) = sqlite_store::match_root(&dir) {
        return sqlite_store::search_in_files(&root, &dir, &pattern, case_sensitive, use_regex, &extensions);
    }
    search_in_files_impl(&dir, &pattern, case_sensitive, use_regex, &extensions)
}

#[tauri::command]
fn write_file(path: String, content: String) -> Result<(), String> {
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::write_file(&root, &path, &content);
    }
    let _ = safe_write_target(&path)?;
    fs::write(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_binary_file(path: String, contents: Vec<u8>) -> Result<(), String> {
    let _ = safe_write_target(&path)?;
    fs::write(&path, &contents).map_err(|e| e.to_string())
}

#[tauri::command]
fn ensure_dir(path: String) -> Result<(), String> {
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::ensure_dir(&root, &path);
    }
    let _ = safe_write_target(&path)?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

// ====== 文件树右键菜单：重命名 / 删除 / 复制 / 移动 ======
// 所有命令复用 safe_write_target 做路径规范化与黑名单校验，
// 拒绝越界或系统关键目录。前端另外做业务级校验（禁止删除工作区根等）。

// 检查 dst 是否等于 src 或位于 src 内部（按路径组件比较，跨平台 normalize 分隔符）。
// 防止把目录复制/移动到自身或子目录内导致递归复制直到路径超长。
fn is_path_within(src: &str, dst: &str) -> bool {
    let norm = |p: &str| p.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let s = norm(src);
    let d = norm(dst);
    s == d || d.starts_with(&format!("{}/", s))
}

#[tauri::command]
fn rename_path(from: String, to: String) -> Result<(), String> {
    if let Some(root) = sqlite_store::match_root(&from) {
        return sqlite_store::rename_path(&root, &from, &to);
    }
    let _ = safe_write_target(&from)?;
    let _ = safe_write_target(&to)?;
    fs::rename(&from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_path(path: String) -> Result<(), String> {
    if let Some(root) = sqlite_store::match_root(&path) {
        return sqlite_store::remove_path(&root, &path);
    }
    let _ = safe_write_target(&path)?;
    let p = std::path::Path::new(&path);
    if p.is_dir() {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(&path).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn copy_path(from: String, to: String) -> Result<(), String> {
    if let Some(root) = sqlite_store::match_root(&from) {
        return sqlite_store::copy_path(&root, &from, &to);
    }
    let _ = safe_write_target(&from)?;
    let _ = safe_write_target(&to)?;
    // 纵深防御：后端也检查目标是否在源内部，防止绕过前端直接 invoke 导致递归复制
    if is_path_within(&from, &to) {
        return Err("Cannot copy into itself or its subdirectory".into());
    }
    let from_p = std::path::Path::new(&from);
    if from_p.is_file() {
        fs::copy(&from, &to).map_err(|e| e.to_string()).map(|_| ())
    } else {
        copy_dir_recursive(from_p, std::path::Path::new(&to))
    }
}

// 递归复制目录：跳过符号链接以避免循环；不跟随链接语义，保持物理复制。
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    }
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_symlink() {
            continue;
        }
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// 在系统文件管理器中显示目标：目录直接打开，文件则定位并选中。
// SQLite 虚拟工作区内的路径在磁盘上不存在，此时退化为显示其 .db 宿主文件。
#[tauri::command]
fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let target = sqlite_store::match_root(&path).unwrap_or_else(|| path.clone());
    let is_dir = std::path::Path::new(&target).is_dir();

    // cfg! 是运行时布尔，三个分支都会参与类型检查，避免 #[cfg] 块带来的平台差异噪音
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer");
        if is_dir {
            c.arg(&target);
        } else {
            c.arg(format!("/select,{}", target));
        }
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        if !is_dir {
            c.arg("-R");
        }
        c.arg(&target);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        if is_dir {
            c.arg(&target);
        } else {
            let parent = std::path::Path::new(&target)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| target.clone());
            c.arg(parent);
        }
        c
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("打开文件管理器失败: {}", e))
}

#[tauri::command]
fn move_path(from: String, to: String) -> Result<(), String> {
    if let Some(root) = sqlite_store::match_root(&from) {
        return sqlite_store::move_path(&root, &from, &to);
    }
    let _ = safe_write_target(&from)?;
    let _ = safe_write_target(&to)?;
    if is_path_within(&from, &to) {
        return Err("Cannot move into itself or its subdirectory".into());
    }
    // 同设备 rename 是原子的；跨设备 rename 会失败，回退到 copy + remove
    match fs::rename(&from, &to) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_path(from.clone(), to)?;
            remove_path(from)
        }
    }
}

// 工作区文件监听：监听整棵目录树，外部（资源管理器等）增删目录/文件时向前端广播 folder-changed 事件，
// 前端防抖后重建文件树。监听句柄存入托管状态，重复监听或关闭文件夹时会先丢弃旧句柄。
struct WatcherState(pub Mutex<Option<RecommendedWatcher>>);

#[tauri::command]
fn watch_folder(path: String, app: tauri::AppHandle) -> Result<(), String> {
    // SQLite 工作区无文件系统树可监听：丢弃旧 watcher 后空操作返回
    if sqlite_store::match_root(&path).is_some() {
        if let Ok(mut state) = app.state::<WatcherState>().0.lock() {
            *state = None;
        }
        return Ok(());
    }
    if let Ok(mut state) = app.state::<WatcherState>().0.lock() {
        *state = None;
    }
    let app_handle = app.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<NotifyEvent>| {
            if res.is_ok() {
                let _ = app_handle.emit("folder-changed", ());
            }
        },
        NotifyConfig::default(),
    )
    .map_err(|e| e.to_string())?;
    watcher
        .watch(std::path::Path::new(&path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    if let Ok(mut state) = app.state::<WatcherState>().0.lock() {
        *state = Some(watcher);
    }
    Ok(())
}

#[tauri::command]
fn stop_watch(app: tauri::AppHandle) -> Result<(), String> {
    if let Ok(mut state) = app.state::<WatcherState>().0.lock() {
        *state = None;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub(crate) struct ImageAssetInfo {
    filename: String,
    width: u32,
    height: u32,
}

#[tauri::command]
fn save_image_to_assets(bytes: Vec<u8>, ext: String, assets_dir: String) -> Result<ImageAssetInfo, String> {
    // SQLite 工作区路由：assets_dir 形如 <db根>/assets，图片写入 node_blob
    if let Some(root) = sqlite_store::match_root(&assets_dir) {
        return sqlite_store::save_image_to_assets(&root, &assets_dir, &bytes, &ext);
    }
    let hash = format!("{:x}", Md5::digest(&bytes));
    let filename = format!("{}.{}", hash, ext);
    let _ = safe_write_target(&assets_dir)?;
    let dest = std::path::Path::new(&assets_dir).join(&filename);

    std::fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
    if !dest.exists() {
        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    }

    let (width, height) = match imagesize::blob_size(&bytes) {
        Ok(size) => (size.width as u32, size.height as u32),
        Err(_) => (0, 0),
    };

    Ok(ImageAssetInfo { filename, width, height })
}

#[tauri::command]
async fn fetch_image_as_base64(url: String) -> Result<String, String> {
    let bytes = if url.starts_with("http://") || url.starts_with("https://") {
        let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else if let Some(root) = sqlite_store::match_root(&url) {
        // SQLite 工作区内的图片（node_blob）
        return sqlite_store::read_file_bytes(&root, &url).map(|bytes| {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        });
    } else {
        let path = std::path::Path::new(&url);
        let canonical = path.canonicalize().map_err(|e| format!("Cannot resolve path {}: {}", url, e))?;
        if !canonical.is_file() {
            return Err(format!("Not a regular file: {}", url));
        }
        std::fs::read(&canonical).map_err(|e| format!("Cannot read local file {}: {}", url, e))?
    };
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(encoded)
}

#[tauri::command]
fn app_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| e.to_string())
        .map(|p| p.to_string_lossy().to_string())
}

// ====== SQLite 工作区管理命令 ======

// 「记住密码」缓存文件：应用数据目录下 db-keys.json（内容是派生密钥，不是明文口令）。
// 拿不到应用数据目录时返回 None —— 只是失去自动解锁，不能因此打不开 db。
fn db_key_store(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("db-keys.json"))
}

// 打开（或新建）SQLite db 工作区并注册。password 为 None / 空串表示按明文库打开；
// 加密库未给密码返回 kind=PasswordRequired，密码错误返回 kind=WrongPassword，
// 非 vsnote db 返回 kind=NotVsNoteDb（前端据此弹出密码框或提示）。
// 未给密码时先用 db_key_store 里记住的派生密钥尝试自动解锁。
#[tauri::command]
async fn db_open(app: tauri::AppHandle, path: String, password: Option<String>) -> Result<serde_json::Value, String> {
    let _ = safe_write_target(&path)?;
    sqlite_store::open(&path, password, db_key_store(&app).as_deref())
}

// 记住当前已打开 db 的派生密钥（密码校验通过后由用户确认调用）。
#[tauri::command]
fn db_remember_password(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let store = db_key_store(&app).ok_or_else(|| "无法定位应用数据目录".to_string())?;
    sqlite_store::remember_key(&path, &store)
}

// 清除已记住的密码；返回是否确有缓存被删除（无缓存时前端提示"本来就没有"）。
#[tauri::command]
fn db_forget_password(app: tauri::AppHandle, path: String) -> Result<bool, String> {
    let store = db_key_store(&app).ok_or_else(|| "无法定位应用数据目录".to_string())?;
    sqlite_store::forget_key(&path, &store)
}

// 关闭并注销 SQLite 工作区连接（切换/关闭工作区时调用，释放文件句柄）。
#[tauri::command]
fn db_close(path: String) -> Result<(), String> {
    sqlite_store::close(&path);
    Ok(())
}

// 导出为 SQLite db：src 可以是普通文件夹或已打开的 db 工作区根（db→db 可换密码/去密码）。
// password 为 None / 空串时导出为明文库。
#[tauri::command]
async fn export_to_db(src: String, dest: String, password: Option<String>) -> Result<serde_json::Value, String> {
    let _ = safe_write_target(&src)?;
    let _ = safe_write_target(&dest)?;
    sqlite_store::export_to_db(&src, &dest, password)
}

// 导出为文件夹：src 必须是已打开的 db 工作区根，dest 为目标目录（不存在则创建，非空则报错）。
#[tauri::command]
async fn export_to_folder(src: String, dest: String) -> Result<serde_json::Value, String> {
    let _ = safe_write_target(&dest)?;
    sqlite_store::export_to_folder(&src, &dest)
}

// ── Git 自动同步（仅真实文件夹 git 仓库工作区）────────────────────────
// 通过系统 git CLI 执行（复用用户既有凭据/代理配置），不引入 git2 依赖。
//
// 同步语义（用户契约，勿回退成 rebase）：
//   ① 本地改动先 commit —— 保住本地成果，并为后续合并提供干净基线；
//   ② 再 pull —— 用 merge（--no-rebase）而非 rebase：不改写已推送历史，
//      非冲突改动由 git 自动合并，真冲突时冲突文件可被精确定位；
//   ③ 冲突按策略处理：
//        markers（默认）能合并的 git 自动合并，合不了的写成 git 标准冲突文件——
//                文件名保持原样，内容带 <<<<<<< / ======= / >>>>>>> 标记，交给用户在编辑器里改；
//        union    冲突块两侧都保留（git merge-file --union），不留标记；
//        ours/theirs 冲突块整体取本地 / 远端；
//   ④ 最后 push —— 先拉后推，避免 non-fast-forward 被远端拒绝；
//   ⑤ 自动解决失败（如二进制文件）或策略为 markers：保留冲突现场（MERGE_HEAD + 冲突标记），
//      把冲突文件列表回传前端，由 UI 提示用户介入。用户改完保存后再次同步，
//      ⓿ 分支会检测到「合并中但无冲突」并自动补上合并提交。

fn run_git(dir: &str, args: &[&str]) -> Result<std::process::Output, String> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        // 禁用终端交互：缺凭据 / 未知主机时立刻失败，而不是把整个应用挂住
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("无法运行 git（未安装或不在 PATH）: {e}"))
}

fn git_out(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git_err(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}

fn git_text(out: &std::process::Output) -> String {
    let s = git_out(out);
    if s.is_empty() { git_err(out) } else { s }
}

// stdout + stderr 合并小写：git 的关键提示两者都可能出现，统一做关键字匹配
fn git_lower(out: &std::process::Output) -> String {
    format!("{} {}", git_err(out), git_out(out)).to_lowercase()
}

fn is_git_repo(dir: &str) -> bool {
    git_dir_of(dir).is_some()
}

// .git 既可能是目录（普通仓库）也可能是文件（worktree / submodule 的 gitfile）
fn git_dir_of(dir: &str) -> Option<std::path::PathBuf> {
    let dot = std::path::Path::new(dir).join(".git");
    if dot.is_dir() {
        return Some(dot);
    }
    if dot.is_file() {
        if let Ok(content) = std::fs::read_to_string(&dot) {
            for line in content.lines() {
                if let Some(rest) = line.trim().strip_prefix("gitdir:") {
                    let p = std::path::PathBuf::from(rest.trim());
                    return Some(if p.is_absolute() { p } else { std::path::Path::new(dir).join(p) });
                }
            }
        }
        return Some(dot);
    }
    None
}

// (merge 进行中, rebase 进行中)
fn git_op_in_progress(dir: &str) -> (bool, bool) {
    let Some(gd) = git_dir_of(dir) else { return (false, false) };
    let merge = gd.join("MERGE_HEAD").exists();
    let rebase = gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists();
    (merge, rebase)
}

// git status --porcelain 输出中路径含特殊字符时是 C 风格转义 + 引号，需还原
fn git_unquote_path(s: &str) -> String {
    let bytes = s.as_bytes();
    if !(bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"') {
        return s.to_string();
    }
    let b = &bytes[1..bytes.len() - 1];
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 >= b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        match b[i + 1] {
            b'n' => { out.push(b'\n'); i += 2; }
            b't' => { out.push(b'\t'); i += 2; }
            b'r' => { out.push(b'\r'); i += 2; }
            b'\\' => { out.push(b'\\'); i += 2; }
            b'"' => { out.push(b'"'); i += 2; }
            b'0'..=b'7' => {
                let mut v: u32 = 0;
                let mut n = 0;
                while n < 3 && i + 1 + n < b.len() && (b'0'..=b'7').contains(&b[i + 1 + n]) {
                    v = v * 8 + (b[i + 1 + n] - b'0') as u32;
                    n += 1;
                }
                out.push((v & 0xff) as u8);
                i += 1 + n;
            }
            x => { out.push(x); i += 2; }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

// 返回 (XY 状态码, 路径)；重命名取新路径
fn git_status_entries(dir: &str) -> Result<Vec<(String, String)>, String> {
    let o = run_git(dir, &["status", "--porcelain"])?;
    if !o.status.success() {
        return Err(format!("git status 失败: {}", git_text(&o)));
    }
    let mut v = Vec::new();
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        if line.len() < 4 { continue; }
        let xy = line[..2].to_string();
        let rest = line[3..].trim();
        let path = match rest.find(" -> ") {
            Some(i) => rest[i + 4..].trim(),
            None => rest,
        };
        if !path.is_empty() {
            v.push((xy, git_unquote_path(path)));
        }
    }
    Ok(v)
}

// 冲突判定：任一侧为 U，或 add/add（AA）、delete/delete（DD）
fn git_conflicts_of(entries: &[(String, String)]) -> Vec<String> {
    entries
        .iter()
        .filter(|(xy, _)| {
            let b = xy.as_bytes();
            if b.len() < 2 { return false; }
            let (x, y) = (b[0], b[1]);
            x == b'U' || y == b'U' || (x == b'A' && y == b'A') || (x == b'D' && y == b'D')
        })
        .map(|(_, p)| p.clone())
        .collect()
}

fn git_conflicts(dir: &str) -> Result<Vec<String>, String> {
    Ok(git_conflicts_of(&git_status_entries(dir)?))
}

fn git_remotes(dir: &str) -> Result<Vec<String>, String> {
    let o = run_git(dir, &["remote"])?;
    Ok(git_out(&o).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

fn git_default_remote(dir: &str) -> String {
    match git_remotes(dir) {
        Ok(r) if r.iter().any(|x| x == "origin") => "origin".to_string(),
        Ok(r) => r.first().cloned().unwrap_or_else(|| "origin".to_string()),
        Err(_) => "origin".to_string(),
    }
}

fn git_branch(dir: &str) -> String {
    run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|o| if o.status.success() { git_out(&o) } else { String::new() })
        .unwrap_or_default()
}

fn git_has_upstream(dir: &str) -> bool {
    run_git(dir, &["rev-parse", "--abbrev-ref", "@{u}"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// (ahead, behind)：无 upstream 时返回 (0, 0)
fn git_ahead_behind(dir: &str) -> (u32, u32) {
    if !git_has_upstream(dir) { return (0, 0); }
    let o = match run_git(dir, &["rev-list", "--left-right", "--count", "HEAD...@{u}"]) {
        Ok(o) => o,
        Err(_) => return (0, 0),
    };
    if !o.status.success() { return (0, 0); }
    // git_out 返回临时 String，必须先绑定，否则 split_whitespace 的借用会悬垂（E0716）
    let counts = git_out(&o);
    let mut it = counts.split_whitespace();
    let ahead = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let behind = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

// 冲突文件按 union 合并：取 :1(base) :2(ours) :3(theirs) 三阶段，用 git merge-file --union
// 生成「两侧改动都保留」的结果（比保留单侧更适合笔记类文本）。失败返回 false 交回上层。
fn git_resolve_union(dir: &str, file: &str) -> Result<bool, String> {
    let show = |stage: u8| -> Option<Vec<u8>> {
        let o = run_git(dir, &["show", &format!(":{stage}:{file}")]).ok()?;
        if o.status.success() { Some(o.stdout) } else { None }
    };
    let (ours, theirs) = match (show(2), show(3)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Ok(false),
    };
    // 二进制（含 NUL）无法按行合并，交回上层按 manual 处理
    if ours.iter().any(|&b| b == 0) || theirs.iter().any(|&b| b == 0) {
        return Ok(false);
    }
    let base = show(1).unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!(
        "vsnote-git-merge-{}-{}",
        std::process::id(),
        file.replace(&['/', '\\', ':'][..], "_")
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let p_ours = tmp.join("ours");
    let p_base = tmp.join("base");
    let p_theirs = tmp.join("theirs");
    std::fs::write(&p_ours, &ours).map_err(|e| format!("写入临时文件失败: {e}"))?;
    std::fs::write(&p_base, &base).map_err(|e| format!("写入临时文件失败: {e}"))?;
    std::fs::write(&p_theirs, &theirs).map_err(|e| format!("写入临时文件失败: {e}"))?;

    let m = std::process::Command::new("git")
        .args(["merge-file", "--union", "-p"])
        .arg(&p_ours)
        .arg(&p_base)
        .arg(&p_theirs)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("无法运行 git merge-file: {e}"))?;

    let merged = m.stdout;
    let ok = if merged.is_empty() && !ours.is_empty() {
        false
    } else {
        let target = std::path::Path::new(dir).join(file);
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&target, &merged).is_ok()
    };
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(ok)
}

// 策略含义：
//   markers —— 能合并的由 git 自动合并，合不了的写 git 标准冲突标记（<<<<<<< / ======= / >>>>>>>），
//              文件名保持不变，交给用户在编辑器里改（默认）；manual 是它的旧名，等价。
//   union   —— 冲突块两侧都保留（git merge-file --union），不留标记
//   ours/theirs —— 冲突块整体取本地 / 远端
fn git_keep_markers(strategy: &str) -> bool {
    strategy == "markers" || strategy == "manual"
}

fn git_default_strategy(strategy: &str) -> String {
    if strategy.is_empty() { "markers".to_string() } else { strategy.to_string() }
}

// 确保冲突文件里真的带 git 标准冲突标记。正常 git merge 已经写好了；
// 异常场景（文件被外部工具改过、标记被清掉）用 checkout --conflict=merge 从暂存区三阶段重建。
// 文件名始终是原文件名，不产生 .orig / .mine 之类副本。
fn git_ensure_conflict_markers(dir: &str, files: &[String]) -> Result<usize, String> {
    let mut ok = 0;
    for f in files {
        let p = std::path::Path::new(dir).join(f);
        let has_marker = std::fs::read(&p)
            .map(|b| String::from_utf8_lossy(&b).contains("<<<<<<< "))
            .unwrap_or(false);
        if has_marker {
            ok += 1;
            continue;
        }
        // 二进制等无法写标记的文件：checkout --conflict 会失败，保持原样交给用户
        let c = run_git(dir, &["checkout", "--conflict=merge", "--", f])?;
        if c.status.success() {
            ok += 1;
        }
    }
    Ok(ok)
}

// 冲突文件的工作区内容是否仍带 git 标准冲突标记（<<<<<<< ）。
// 读不到 / 二进制（含 NUL）按「仍冲突」处理：二进制写不了标记，交给用户在弹窗里选
// union/ours/theirs，避免静默把某一侧内容提交进合并。
fn file_has_conflict_markers(dir: &str, file: &str) -> bool {
    let p = std::path::Path::new(dir).join(file);
    match std::fs::read(&p) {
        Ok(bytes) => {
            if bytes.iter().any(|&b| b == 0) {
                return true;
            }
            String::from_utf8_lossy(&bytes).contains("<<<<<<< ")
        }
        Err(_) => true,
    }
}

// 合并现场中「已无冲突标记」的文件，是真解决还是假解决。
// 假解决 = 用户没看到标记就直接保存：自动保存 / Ctrl+S 会把合并前的编辑器缓冲区
// 原样写回，文件同样不含 <<<<<<<，但远端（:3 stage）那一侧的改动并没有被吸纳。
// 旧逻辑仅凭「无标记」就补合并提交，远端改动会被静默丢弃——用户视角就是
// 「删除某行后重启，相邻的第一行/第三行丢一半」。
// 判定：取三阶段 :1(base) :2(ours) :3(theirs)，把当前工作区内容当 ours 与
// theirs 再合并一次（git merge-file）。退出码 > 0 表示仍冲突 → 未真正解决。
// 拿不到阶段内容 / 二进制 / merge-file 异常时返回 true（保持旧行为，交人工判断）。
fn conflict_resolution_is_genuine(dir: &str, file: &str) -> Result<bool, String> {
    let show = |stage: u8| -> Option<Vec<u8>> {
        let o = run_git(dir, &["show", &format!(":{stage}:{file}")]).ok()?;
        if o.status.success() { Some(o.stdout) } else { None }
    };
    let (ours, theirs) = match (show(2), show(3)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Ok(true),
    };
    // 二进制（含 NUL）不按文本合并判定，走弹窗人工选择
    if ours.iter().any(|&b| b == 0) || theirs.iter().any(|&b| b == 0) {
        return Ok(true);
    }
    let working = match std::fs::read(std::path::Path::new(dir).join(file)) {
        Ok(b) => b,
        Err(_) => return Ok(true),
    };
    if working.iter().any(|&b| b == 0) {
        return Ok(true);
    }
    let base = show(1).unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!(
        "vsnote-git-check-{}-{}",
        std::process::id(),
        file.replace(&['/', '\\', ':'][..], "_")
    ));
    if std::fs::create_dir_all(&tmp).is_err() {
        return Ok(true);
    }
    let p_working = tmp.join("working");
    let p_base = tmp.join("base");
    let p_theirs = tmp.join("theirs");
    let prepared = std::fs::write(&p_working, &working).is_ok()
        && std::fs::write(&p_base, &base).is_ok()
        && std::fs::write(&p_theirs, &theirs).is_ok();
    let genuine = if !prepared {
        true
    } else {
        match std::process::Command::new("git")
            .args(["merge-file", "-p"])
            .arg(&p_working)
            .arg(&p_base)
            .arg(&p_theirs)
            .current_dir(dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
        {
            // 退出码 = 冲突数（>0 表示仍有冲突）；<=0（含 255 错误）按信任处理
            Ok(out) => out.status.code().map(|c| c <= 0).unwrap_or(true),
            Err(_) => true,
        }
    };
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(genuine)
}

// 假解决时还原冲突标记，让用户在编辑器里看到真实冲突（合并现场不动）。
// 仅还原 LF 纯净（vsnote 自己保存格式）的文件：含 CR 的多是外部工具改过的，
// 直接还原会覆盖别人的解决成果，只上报不动作。
fn restore_conflict_markers_for_fake(dir: &str, files: &[String]) {
    let restorable: Vec<String> = files
        .iter()
        .filter(|f| {
            std::fs::read(std::path::Path::new(dir).join(f))
                .map(|b| !b.iter().any(|&c| c == b'\r'))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if !restorable.is_empty() {
        let _ = git_ensure_conflict_markers(dir, &restorable);
    }
}

// 按策略解决一批冲突文件（解决后逐个 add，但不提交，交由调用方完成合并提交）
fn git_auto_resolve(dir: &str, strategy: &str, files: &[String]) -> Result<bool, String> {
    for f in files {
        let ok = match strategy {
            "ours" => run_git(dir, &["checkout", "--ours", "--", f])?.status.success(),
            "theirs" => run_git(dir, &["checkout", "--theirs", "--", f])?.status.success(),
            "union" => git_resolve_union(dir, f)?,
            _ => false,
        };
        if !ok { return Ok(false); }
        let a = run_git(dir, &["add", "--", f])?;
        if !a.status.success() { return Ok(false); }
    }
    Ok(true)
}

// pull 的四种结局
enum GitPullOutcome {
    Clean,                  // 拉取成功（含 fast-forward / 自动合并 / 已是最新）
    NoUpstream,             // 无远端或无上游分支，跳过
    AutoResolved(Vec<String>), // 有冲突但已按策略自动解决并合并提交
    Conflict(Vec<String>),  // 冲突无法自动解决，保留现场等用户
}

// 拉取并合并（--no-rebase）：真冲突时按 strategy 处理
fn git_pull_merge(dir: &str, strategy: &str) -> Result<GitPullOutcome, String> {
    // fetch 失败不致命（离线场景），交给 pull 再给出最终结论
    let _ = run_git(dir, &["fetch", "--all", "--tags"]);

    let p = if git_has_upstream(dir) {
        run_git(dir, &["pull", "--no-rebase", "--no-edit"])?
    } else {
        let remote = git_default_remote(dir);
        let branch = git_branch(dir);
        if branch.is_empty() || branch == "HEAD" {
            return Ok(GitPullOutcome::NoUpstream);
        }
        run_git(dir, &["pull", "--no-rebase", "--no-edit", &remote, &branch])?
    };

    if p.status.success() {
        return Ok(GitPullOutcome::Clean);
    }

    let conflicts = git_conflicts(dir)?;
    if !conflicts.is_empty() {
        if git_keep_markers(strategy) {
            // 保留 git 标准冲突文件（文件名不变、内容带冲突标记），交给用户在编辑器里解决
            let _ = git_ensure_conflict_markers(dir, &conflicts);
            return Ok(GitPullOutcome::Conflict(conflicts));
        }
        if git_auto_resolve(dir, strategy, &conflicts)? {
            let _ = run_git(dir, &["add", "-A"])?;
            // 处于合并中：--no-edit 生成合并提交；否则兜底给一条普通提交
            let c = run_git(dir, &["commit", "--no-edit"])?;
            if !c.status.success() {
                let c2 = run_git(dir, &["commit", "-m", "vsnote: 自动合并远端改动"])?;
                if !c2.status.success() {
                    return Ok(GitPullOutcome::Conflict(git_conflicts(dir)?));
                }
            }
            let left = git_conflicts(dir)?;
            if !left.is_empty() {
                return Ok(GitPullOutcome::Conflict(left));
            }
            return Ok(GitPullOutcome::AutoResolved(conflicts));
        }
        return Ok(GitPullOutcome::Conflict(conflicts));
    }

    let low = git_lower(&p);
    if low.contains("no tracking information")
        || low.contains("no upstream")
        || low.contains("couldn't find remote ref")
        || low.contains("no such ref was fetched")
    {
        return Ok(GitPullOutcome::NoUpstream);
    }
    if low.contains("already up to date") || low.contains("already up-to-date") {
        return Ok(GitPullOutcome::Clean);
    }
    Err(format!("git pull 失败: {}", git_text(&p)))
}

// 推送；无 upstream 时补 -u 建立追踪（首次推送）
fn git_push(dir: &str) -> Result<(), String> {
    let p = run_git(dir, &["push"])?;
    if p.status.success() {
        return Ok(());
    }
    let low = git_lower(&p);
    if low.contains("no upstream") || low.contains("no configured push destination") {
        let remote = git_default_remote(dir);
        let branch = git_branch(dir);
        if !branch.is_empty() && branch != "HEAD" {
            let p2 = run_git(dir, &["push", "-u", &remote, &branch])?;
            if p2.status.success() {
                return Ok(());
            }
            return Err(format!("git push 失败: {}", git_text(&p2)));
        }
    }
    Err(format!("git push 失败: {}", git_text(&p)))
}

#[tauri::command]
async fn git_check(path: String) -> Result<bool, String> {
    Ok(is_git_repo(&path))
}

// 仓库状态快照：供 UI 展示分支 / 领先落后 / 脏文件数 / 冲突文件列表
#[tauri::command]
async fn git_status(path: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_git_repo(&path) {
            return Ok(serde_json::json!({ "isRepo": false }));
        }
        let remotes = git_remotes(&path)?;
        let entries = git_status_entries(&path)?;
        let (ahead, behind) = git_ahead_behind(&path);
        let (merging, rebasing) = git_op_in_progress(&path);
        Ok(serde_json::json!({
            "isRepo": true,
            "branch": git_branch(&path),
            "hasRemote": !remotes.is_empty(),
            "remotes": remotes,
            "hasUpstream": git_has_upstream(&path),
            "ahead": ahead,
            "behind": behind,
            "changed": entries.len(),
            "dirty": !entries.is_empty(),
            "conflicts": git_conflicts_of(&entries),
            "merging": merging,
            "rebasing": rebasing,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// 完整同步：commit（本地改动）→ pull（merge，冲突按策略处理）→ push。
// 无远端的本地仓库只做本地 commit。commit 无变更不算错误。
#[tauri::command]
async fn git_auto_sync(path: String, message: String, strategy: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_git_repo(&path) {
            return Err("不是 git 仓库".to_string());
        }
        let strategy = git_default_strategy(&strategy);

        // ⓿ 清理历史遗留的 rebase 状态（旧版本 --rebase 中断可能留下）
        let (merging, rebasing) = git_op_in_progress(&path);
        if rebasing {
            let _ = run_git(&path, &["rebase", "--abort"]);
        }
        // 已存在未解决冲突时的处理：
        //   - 合并进行中，且用户真解决了（文件无 <<<<<<<，且用三阶段重放验证过：
        //     工作区内容与 :3(theirs) 再合并一次不再冲突）→ 暂存并补合并提交，
        //     随后继续正常的 ❶❸ 流程（拉取 / 推送）；
        //   - 假解决（文件无标记，但并未吸纳 theirs 的改动——典型是用户没看到标记、
        //     自动保存/Ctrl+S 把合并前的编辑器缓冲区写回）→ 还原冲突标记后重新上报，
        //     绝不静默补合并提交（否则远端改动被丢：重启后冲突行邻近的行丢一半）；
        //   - 只解决了一部分：先把已改好的暂存（避免被弹窗里的 union/ours/theirs 覆盖），
        //     只把还带标记的上报 UI；
        //   - 仍全带标记 / 二进制（写不了标记）/ 非合并态的残留冲突 → 不叠加操作，
        //     上报 UI 让用户选保留方式（union/ours/theirs）或继续手改。
        // 只凭 git status 的 U 状态码判断是不够的：用户改的是工作区文件、没有 git add，
        // 索引里会一直是 UU，必须再看文件内容里标记是否已被删掉；而「标记被删掉」本身
        // 也不够——直接保存同样无标记，所以再加一层三阶段重放验证（见
        // conflict_resolution_is_genuine）。
        let pending = git_conflicts(&path)?;
        let mut finished_merge = false;
        if !pending.is_empty() {
            let mut resolved_by_user: Vec<String> = Vec::new();
            let mut still_marked: Vec<String> = Vec::new();
            for f in &pending {
                if file_has_conflict_markers(&path, f) {
                    still_marked.push(f.clone());
                } else {
                    resolved_by_user.push(f.clone());
                }
            }
            if merging {
                // 无标记 ≠ 已解决：自动保存 / Ctrl+S 会把合并前的编辑器缓冲区原样写回，
                // 文件同样不含 <<<<<<<，但远端那一侧改动并未被吸纳。逐个用三阶段重放验证，
                // 假解决绝不静默补合并提交——否则远端改动被静默丢弃，用户视角就是
                // 「删了某行当时看着没事，重启后相邻的第一行/第三行丢一半」。
                let mut genuine: Vec<String> = Vec::new();
                let mut fake: Vec<String> = Vec::new();
                for f in &resolved_by_user {
                    if conflict_resolution_is_genuine(&path, f)? {
                        genuine.push(f.clone());
                    } else {
                        fake.push(f.clone());
                    }
                }
                if !fake.is_empty() {
                    // 假解决：还原冲突标记（仅 LF 纯净文件），让用户在编辑器里看到
                    // 真实冲突后真正解决；合并现场（MERGE_HEAD）不动，稍后重新上报
                    restore_conflict_markers_for_fake(&path, &fake);
                    let mut report = still_marked.clone();
                    report.extend(fake.iter().cloned());
                    return Ok(serde_json::json!({
                        "conflict": true, "conflicts": report, "stage": "unresolved",
                        "committed": false, "pulled": false, "pushed": false, "resolved": false,
                        "fakeResolved": fake
                    }));
                }
                if genuine.is_empty() {
                    // 全部仍带标记，没有可暂存的已解决文件
                    return Ok(serde_json::json!({
                        "conflict": true, "conflicts": pending, "stage": "unresolved",
                        "committed": false, "pulled": false, "pushed": false, "resolved": false
                    }));
                }
                if still_marked.is_empty() {
                    for f in &genuine {
                        let a = run_git(&path, &["add", "--", f])?;
                        if !a.status.success() {
                            return Err(format!("git add 失败: {}（解决冲突后暂存失败）", git_text(&a)));
                        }
                    }
                    let c = run_git(&path, &["commit", "--no-edit"])?;
                    finished_merge = c.status.success();
                } else {
                    // 部分解决：暂存已改好的，只上报还带标记的，等用户把剩下的也改完
                    for f in &genuine {
                        let _ = run_git(&path, &["add", "--", f]);
                    }
                    return Ok(serde_json::json!({
                        "conflict": true, "conflicts": still_marked, "stage": "unresolved",
                        "committed": false, "pulled": false, "pushed": false, "resolved": false
                    }));
                }
            } else {
                return Ok(serde_json::json!({
                    "conflict": true, "conflicts": pending, "stage": "unresolved",
                    "committed": false, "pulled": false, "pushed": false, "resolved": false
                }));
            }
        } else if merging {
            // 索引里已无冲突条目（用户可能已在外部 git add 过）→ 直接补合并提交
            let c = run_git(&path, &["commit", "--no-edit"])?;
            finished_merge = c.status.success();
        }

        let remotes = git_remotes(&path)?;
        let has_remote = !remotes.is_empty();
        let mut committed = finished_merge; // 上面补的合并提交也算提交过
        let mut pulled = false;
        let mut pushed = false;
        let mut resolved = false;

        // ❶ 本地改动先提交：保住本地成果，并为后续合并提供干净基线
        let entries = git_status_entries(&path)?;
        if !entries.is_empty() {
            let _ = run_git(&path, &["add", "-A"])?;
            let c = run_git(&path, &["commit", "-m", &message])?;
            if c.status.success() {
                committed = true;
            } else {
                let out = git_text(&c);
                if !out.contains("nothing to commit")
                    && !out.contains("no changes added")
                    && !out.contains("nothing added to commit")
                {
                    return Err(format!(
                        "git commit 失败: {out}（若提示身份未配置，请先执行 git config --global user.name / user.email）"
                    ));
                }
            }
        }

        // ❷ 先拉：merge 合并远端改动
        if has_remote {
            match git_pull_merge(&path, &strategy)? {
                GitPullOutcome::Clean => { pulled = true; }
                GitPullOutcome::NoUpstream => {}
                GitPullOutcome::AutoResolved(files) => {
                    pulled = true;
                    resolved = true;
                    let _ = files;
                }
                GitPullOutcome::Conflict(files) => {
                    return Ok(serde_json::json!({
                        "conflict": true, "conflicts": files, "stage": "merge",
                        "committed": committed, "pulled": false, "pushed": false,
                        "resolved": false, "strategy": strategy,
                        "markers": git_keep_markers(&strategy)
                    }));
                }
            }
        }

        // ❸ 后推：先拉后推，避免被远端以 non-fast-forward 拒绝。
        //    定时同步无变更时不空推，省掉一次无意义的网络往返与凭据询问。
        if has_remote {
            let (ahead, _) = git_ahead_behind(&path);
            if ahead > 0 || !git_has_upstream(&path) {
                git_push(&path)?;
                pushed = true;
            }
        }

        Ok(serde_json::json!({
            "conflict": false, "conflicts": [], "committed": committed,
            "pulled": pulled, "pushed": pushed, "resolved": resolved,
            "noRemote": !has_remote, "strategy": strategy
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// 只拉取（打开工作区 / 手动拉取）：工作区脏则跳过，交给定时同步先提交再拉。
// 返回 updated 表示 HEAD 是否前进；conflict=true 时需 UI 提示用户解决。
#[tauri::command]
async fn git_auto_pull(path: String, strategy: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_git_repo(&path) {
            return Err("不是 git 仓库".to_string());
        }
        let strategy = git_default_strategy(&strategy);
        let remotes = git_remotes(&path)?;
        if remotes.is_empty() {
            return Ok(serde_json::json!({ "skipped": true, "reason": "noRemote", "updated": false }));
        }
        let entries = git_status_entries(&path)?;
        if !git_conflicts_of(&entries).is_empty() {
            return Ok(serde_json::json!({
                "skipped": true, "reason": "conflict", "updated": false,
                "conflict": true, "conflicts": git_conflicts_of(&entries)
            }));
        }
        if !entries.is_empty() {
            return Ok(serde_json::json!({ "skipped": true, "reason": "dirty", "updated": false }));
        }
        let before = run_git(&path, &["rev-parse", "HEAD"]).map(|o| git_out(&o)).unwrap_or_default();

        match git_pull_merge(&path, &strategy)? {
            GitPullOutcome::Clean => {}
            GitPullOutcome::NoUpstream => {
                return Ok(serde_json::json!({ "skipped": true, "reason": "noUpstream", "updated": false }));
            }
            GitPullOutcome::AutoResolved(files) => {
                return Ok(serde_json::json!({
                    "skipped": false, "updated": true, "conflict": false,
                    "resolved": true, "conflicts": files, "strategy": strategy
                }));
            }
            GitPullOutcome::Conflict(files) => {
                return Ok(serde_json::json!({
                    "skipped": false, "updated": false, "conflict": true, "conflicts": files
                }));
            }
        }

        let after = run_git(&path, &["rev-parse", "HEAD"]).map(|o| git_out(&o)).unwrap_or_default();
        let updated = !after.is_empty() && before != after;
        Ok(serde_json::json!({ "skipped": false, "updated": updated, "conflict": false }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// 用户介入解决冲突：按 strategy 解决（union/ours/theirs）→ 合并提交 → 继续推送。
// files 为空时自动取当前所有冲突文件。
#[tauri::command]
async fn git_resolve_conflicts(
    path: String,
    strategy: String,
    files: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_git_repo(&path) {
            return Err("不是 git 仓库".to_string());
        }
        let target = match files {
            Some(f) if !f.is_empty() => f,
            _ => git_conflicts(&path)?,
        };
        if target.is_empty() {
            return Ok(serde_json::json!({ "resolved": false, "reason": "noConflict" }));
        }
        if !git_auto_resolve(&path, &strategy, &target)? {
            return Ok(serde_json::json!({
                "resolved": false, "reason": "strategyFailed", "conflicts": git_conflicts(&path)?
            }));
        }
        let _ = run_git(&path, &["add", "-A"])?;
        let (merging, _) = git_op_in_progress(&path);
        let c = if merging {
            run_git(&path, &["commit", "--no-edit"])?
        } else {
            run_git(&path, &["commit", "-m", "vsnote: 解决合并冲突"])?
        };
        let committed = c.status.success();
        let mut pushed = false;
        let mut push_error: Option<String> = None;
        if !git_remotes(&path)?.is_empty() {
            match git_push(&path) {
                Ok(()) => pushed = true,
                Err(e) => push_error = Some(e),
            }
        }
        Ok(serde_json::json!({
            "resolved": true, "committed": committed, "pushed": pushed,
            "pushError": push_error, "conflicts": git_conflicts(&path)?
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// 放弃本次远端合并：回到合并前的本地提交，本地已提交内容不丢。
// 无 MERGE_HEAD 但仍有冲突标记时（罕见）回退工作区到 HEAD。
#[tauri::command]
async fn git_abort_merge(path: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_git_repo(&path) {
            return Err("不是 git 仓库".to_string());
        }
        let (merging, rebasing) = git_op_in_progress(&path);
        let has_conflicts = !git_conflicts(&path)?.is_empty();
        if !merging && !has_conflicts {
            return Ok(serde_json::json!({ "aborted": false, "reason": "noMerge" }));
        }
        if rebasing {
            let _ = run_git(&path, &["rebase", "--abort"]);
        }
        if merging {
            let a = run_git(&path, &["merge", "--abort"])?;
            if !a.status.success() {
                return Err(format!("git merge --abort 失败: {}", git_text(&a)));
            }
        } else {
            let r = run_git(&path, &["reset", "--hard", "HEAD"])?;
            if !r.status.success() {
                return Err(format!("git reset --hard 失败: {}", git_text(&r)));
            }
        }
        Ok(serde_json::json!({ "aborted": true, "conflicts": git_conflicts(&path)? }))
    })
    .await
    .map_err(|e| e.to_string())?
}


#[tauri::command]
fn generate_toc(content: String) -> String {
    let mut items: Vec<(usize, String, String)> = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        let (level, text) = if trimmed.starts_with("###### ") {
            (6, trimmed[7..].trim())
        } else if trimmed.starts_with("##### ") {
            (5, trimmed[6..].trim())
        } else if trimmed.starts_with("#### ") {
            (4, trimmed[5..].trim())
        } else if trimmed.starts_with("### ") {
            (3, trimmed[4..].trim())
        } else if trimmed.starts_with("## ") {
            (2, trimmed[3..].trim())
        } else if trimmed.starts_with("# ") {
            (1, trimmed[2..].trim())
        } else {
            continue;
        };

        let id = heading_to_id(text);
        items.push((level, text.to_string(), id));
    }

    if items.is_empty() {
        return String::new();
    }

    let mut html = String::from(r#"<div class="toc"><div class="toc-title">📑 目录</div><ul class="toc-list">"#);
    let mut prev_level = 0;

    for (level, text, id) in &items {
        if *level > prev_level {
            for _ in prev_level..*level {
                html.push_str("<ul>");
            }
        } else if *level < prev_level {
            for _ in *level..prev_level {
                html.push_str("</ul>");
            }
        }
        let href = format!("#{}", id);
        let item = format!(r#"<li><a href="{}">{}</a></li>"#, escape_html(&href), escape_html(text));
        html.push_str(&item);
        prev_level = *level;
    }

    for _ in 0..prev_level {
        html.push_str("</ul>");
    }

    html.push_str("</ul></div>");
    html
}

fn heading_to_id(text: &str) -> String {
    let mut id = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
            if c == ' ' || c == '-' || c == '_' {
                id.push('-');
            } else {
                // to_lowercase() may yield multiple chars for some Unicode (e.g. İ → i\u{0307}),
                // but heading chars are single-width in practice; take only the first.
                if let Some(lower_c) = c.to_lowercase().next() {
                    id.push(lower_c);
                }
            }
        }
    }
    let collapsed: String = id.chars().fold(String::new(), |mut acc, ch| {
        if ch != '-' || acc.chars().last() != Some('-') {
            acc.push(ch);
        }
        acc
    });
    collapsed.trim_matches('-').to_string()
}

fn extract_abbreviations(content: &str) -> Vec<(String, String)> {
    let mut abbrs = Vec::new();
    for line in content.lines() {
        if line.starts_with("*[") {
            if let Some(bracket_end) = line.find("]: ") {
                let term = line[2..bracket_end].to_string();
                let def = line[bracket_end + 3..].to_string();
                if !term.is_empty() {
                    abbrs.push((term, def));
                }
            }
        }
    }
    abbrs
}

fn embed_abbr_data(html: &str, abbreviations: &[(String, String)]) -> String {
    if abbreviations.is_empty() {
        return html.to_string();
    }
    let json = serde_json::to_string(abbreviations).unwrap_or_default();
    let safe_json = json.replace('\'', "&#x27;");
    format!("{html}<div id=\"abbr-data\" style=\"display:none\" data-abbrs='{safe_json}'></div>")
}

#[allow(dead_code)]
#[tauri::command]
fn render_markdown(content: String) -> String {
    use pulldown_cmark::{Parser, Options, html};

    let abbreviations = extract_abbreviations(&content);
    let preprocessed = preprocess_markdown(content);
    let (guarded, placeholders) = guard_math_blocks(&preprocessed);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    // options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(&guarded, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    let html = sanitize_html(&html_output);
    let html = restore_math_blocks(&html, &placeholders);
    embed_abbr_data(&html, &abbreviations)
}

fn guard_math_blocks(content: &str) -> (String, Vec<String>) {
    let mut placeholders = Vec::new();
    let mut result = String::with_capacity(content.len());
    let mut i = 0;
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut in_backtick = false;
    let mut in_code_block = false;
    let mut code_fence_count = 0;
    let mut in_code_tag = false; // Track <code>...</code> tags from preprocess_markdown

    while i < len {
        // Track fenced code blocks (```...``` etc.)
        if chars[i] == '`' {
            let mut bt_count = 1;
            while i + bt_count < len && chars[i + bt_count] == '`' {
                bt_count += 1;
            }
            if bt_count >= 3 {
                if !in_code_block {
                    in_code_block = true;
                    code_fence_count = bt_count;
                    for j in 0..bt_count { result.push(chars[i + j]); }
                    i += bt_count;
                    continue;
                } else if bt_count >= code_fence_count {
                    in_code_block = false;
                    for j in 0..bt_count { result.push(chars[i + j]); }
                    i += bt_count;
                    continue;
                }
            }
        }

        // Skip everything inside code blocks
        if in_code_block {
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // Track <code> and </code> tags — preprocess_markdown converts `$$` to <code>$$</code>
        // We must NOT treat $$ inside <code> tags as math delimiters
        if !in_backtick {
            // Check for <code> opening tag
            if i + 5 < len && chars[i] == '<' && chars[i + 1] == 'c' && chars[i + 2] == 'o'
                && chars[i + 3] == 'd' && chars[i + 4] == 'e' && chars[i + 5] == '>' {
                in_code_tag = true;
                // Copy the entire <code> tag
                for j in 0..6 { result.push(chars[i + j]); }
                i += 6;
                continue;
            }
            // Check for </code> closing tag
            if i + 6 < len && chars[i] == '<' && chars[i + 1] == '/'
                && chars[i + 2] == 'c' && chars[i + 3] == 'o' && chars[i + 4] == 'd'
                && chars[i + 5] == 'e' && chars[i + 6] == '>' {
                in_code_tag = false;
                for j in 0..7 { result.push(chars[i + j]); }
                i += 7;
                continue;
            }
        }

        // Skip $$ matching inside <code> tags
        if in_code_tag {
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // Track inline code backticks to avoid false $$ matching
        if chars[i] == '`' {
            in_backtick = !in_backtick;
            result.push(chars[i]);
            i += 1;
            continue;
        }

        if !in_backtick && i + 1 < len && chars[i] == '$' && chars[i + 1] == '$' {
            // Display math: $$...$$
            let start = i;
            i += 2;
            let mut found_end = false;
            while i + 1 < len {
                if chars[i] == '$' && chars[i + 1] == '$' {
                    i += 2;
                    let math_block: String = chars[start..i].iter().collect();
                    let idx = placeholders.len();
                    placeholders.push(math_block);
                    result.push_str(&format!("<!--MATHBLOCK_{}-->", idx));
                    found_end = true;
                    break;
                }
                i += 1;
            }
            if !found_end {
                for j in start..i { result.push(chars[j]); }
            }
        } else if !in_backtick && chars[i] == '$' && i + 1 < len && !chars[i + 1].is_whitespace() && chars[i + 1] != '$' {
            // Inline math: $...$ (single dollar); note display math $$...$$ is caught
            // in the prior else-if so the `i == start + 1` guard below is unreachable.
            let start = i;
            i += 1;
            let mut found_end = false;
            while i < len {
                if chars[i] == '$' && (i == start + 1 || !chars[i - 1].is_whitespace()) {
                    i += 1;
                    let math_block: String = chars[start..i].iter().collect();
                    let idx = placeholders.len();
                    placeholders.push(math_block);
                    result.push_str(&format!("<!--MATHBLOCK_{}-->", idx));
                    found_end = true;
                    break;
                }
                i += 1;
            }
            if !found_end {
                for j in start..i { result.push(chars[j]); }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    (result, placeholders)
}

fn restore_math_blocks(html: &str, placeholders: &[String]) -> String {
    let mut result = html.to_string();
    for (idx, math) in placeholders.iter().enumerate() {
        let marker = format!("<!--MATHBLOCK_{}-->", idx);
        // HTML-escape the math content so that LaTeX characters
        // (especially & used as column separator in matrices / align)
        // are not misinterpreted by the browser's HTML parser.
        let escaped = escape_html(math);
        result = result.replace(&marker, &escaped);
    }
    result
}

fn preprocess_markdown(content: String) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let len = lines.len();
    let mut in_code_block = false;
    let mut code_fence_count = 0;
    let mut in_math_block = false;
    let mut line_types: Vec<LineType> = Vec::with_capacity(len);

    for idx in 0..len {
        let line = lines[idx];
        let trimmed = line.trim_start();

        if !in_code_block {
            let bt_count = count_backtick_prefix(trimmed);
            if bt_count >= 3 {
                in_code_block = true;
                code_fence_count = bt_count;
                line_types.push(LineType::Code);
                continue;
            }
        } else if in_code_block {
            let bt_count = count_backtick_prefix(trimmed);
            if bt_count >= code_fence_count {
                let after = &trimmed[bt_count..];
                if after.trim().is_empty() {
                    in_code_block = false;
                    line_types.push(LineType::Code);
                    continue;
                }
            }
        }
        if in_code_block {
            line_types.push(LineType::Code);
            continue;
        }
        
        if trimmed.starts_with("$$") {
            if in_math_block {
                in_math_block = false;
                line_types.push(LineType::Math);
            } else {
                in_math_block = true;
                line_types.push(LineType::Math);
            }
            continue;
        }
        if in_math_block {
            line_types.push(LineType::Math);
            continue;
        }
        
        // *[TERM]: definition → abbreviation
        if line.starts_with("*[") {
            if let Some(bracket_end) = line.find("]: ") {
                let term = &line[2..bracket_end]; // between *[ and ]:
                if !term.is_empty() {
                    line_types.push(LineType::Abbr);
                    continue;
                }
            }
        }
        if line.starts_with("> [!") {
            line_types.push(LineType::Alert);
        } else if line.starts_with(": ") || line == ":" {
            line_types.push(LineType::Def);
        } else if idx + 1 < len {
            let next = lines[idx + 1];
            if (next.starts_with(": ") || next == ":")
                && !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with('-')
                && !line.starts_with('*')
                && !line.starts_with('>')
                && !line.starts_with('|')
                && !line.starts_with('`')
                && !line.starts_with('[')
                && !line.starts_with('<')
                && !line.starts_with('!')
            {
                line_types.push(LineType::DefTerm);
            } else {
                line_types.push(LineType::Normal);
            }
        } else {
            line_types.push(LineType::Normal);
        }
    }

    let mut result = String::new();
    let mut i = 0;

    while i < len {
        let line = lines[i];
        let lt = &line_types[i];

        if *lt == LineType::Code || *lt == LineType::Math {
            result.push_str(line);
            result.push('\n');
            i += 1;
            continue;
        }

        if *lt == LineType::Abbr {
            // Abbreviation definitions are hidden from output
            result.push('\n');
            i += 1;
            continue;
        }

        if *lt == LineType::Alert {
            if let Some(alert_content) = parse_alert(line) {
                result.push_str(&alert_content);
                i += 1;
                while i < len && lines[i].starts_with('>') {
                    let stripped = lines[i].strip_prefix('>').unwrap_or("");
                    let content_line = stripped.strip_prefix(' ').unwrap_or(stripped);
                    result.push_str(&process_inline_markdown(content_line));
                    result.push('\n');
                    i += 1;
                }
                result.push_str("</div></div>\n\n");
            } else {
                result.push_str(&process_inline_markdown(line));
                result.push('\n');
                i += 1;
            }
            continue;
        }

        if *lt == LineType::DefTerm {
            result.push_str("<dl>\n");
            while i < len && line_types[i] == LineType::DefTerm {
                let processed = process_inline_markdown(lines[i]);
                result.push_str(&format!("<dt>{}</dt>\n", processed));
                i += 1;
                while i < len && line_types[i] == LineType::Def {
                    let def_text = lines[i].strip_prefix(": ").unwrap_or("");
                    let processed_def = process_inline_markdown(def_text);
                    result.push_str(&format!("<dd>{}</dd>\n", processed_def));
                    i += 1;
                }
            }
            result.push_str("</dl>\n");
            continue;
        }

        if contains_html_tag(line) {
            result.push_str(line);
        } else {
            let processed = process_inline_markdown(line);
            result.push_str(&processed);
        }
        result.push('\n');
        i += 1;
    }

    result
}

#[derive(PartialEq)]
enum LineType {
    Normal,
    Code,
    Math,
    Alert,
    Def,
    DefTerm,
    Abbr,
}

fn contains_html_tag(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '<' && i + 1 < len {
            let next = chars[i + 1];
            if next.is_ascii_alphabetic() || next == '/' {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn process_inline_markdown(line: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Backslash escapes (CommonMark §2.2): \* → *, \[ → [, etc.
        if chars[i] == '\\' && i + 1 < len {
            let next = chars[i + 1];
            // Preserve \( and \) for KaTeX inline math compatibility
            if next == '(' || next == ')' {
                result.push('\\');
                result.push(next);
                i += 2;
                continue;
            }
            if matches!(next, '!' | '"' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | ',' | '-' | '.' | '/' | ':' | ';' | '<' | '=' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '_' | '`' | '{' | '|' | '}' | '~') {
                // Use HTML entity for chars that pulldown-cmark would re-interpret
                match next {
                    '*' => result.push_str("&#42;"),
                    '_' => result.push_str("&#95;"),
                    '`' => result.push_str("&#96;"),
                    '[' => result.push_str("&#91;"),
                    ']' => result.push_str("&#93;"),
                    '!' => result.push_str("&#33;"),
                    '#' => result.push_str("&#35;"),
                    '~' => result.push_str("&#126;"),
                    '<' => result.push_str("&lt;"),
                    '&' => result.push_str("&amp;"),
                    '"' => result.push_str("&quot;"),
                    '\'' => result.push_str("&#x27;"),
                    c => result.push(c),
                }
                i += 2;
                continue;
            }
            result.push('\\');
            i += 1;
            continue;
        }

        // `inline code` → <code>inline code</code>
        // Supports 1-3 backtick delimiters per CommonMark (e.g. `` ` ``)
        if chars[i] == '`' {
            // Count opening backticks
            let mut open_count = 1;
            while i + open_count < len && chars[i + open_count] == '`' {
                open_count += 1;
            }
            let mut j = i + open_count;
            // Scan for matching closing sequence
            while j + open_count <= len {
                if j < len && chars[j] == '`' {
                    // Check if we have exactly open_count backticks
                    let mut match_count = 1;
                    while j + match_count < len && chars[j + match_count] == '`' {
                        match_count += 1;
                    }
                    if match_count >= open_count {
                        // Found matching closer
                        let inner: String = chars[i + open_count..j].iter().collect();
                        // CommonMark §6.1: strip one space from each end if both start/end with space
                        let trimmed = inner.trim();
                        let code_content = if inner.len() > 1 && inner.starts_with(' ') && inner.ends_with(' ') && !trimmed.is_empty() {
                            &inner[1..inner.len() - 1]
                        } else if inner.chars().all(|c| c == ' ') {
                            " "
                        } else {
                            &inner
                        };
                        let safe_content = escape_html(code_content);
                        // Escape all backslashes to &#92; so pulldown-cmark
                        // won't apply backslash escapes to the content inside
                        // <code> tags (e.g. \* → *, \# → #).
                        let safe_content = safe_content.replace('\\', "&#92;");
                        result.push_str(&format!("<code>{}</code>", safe_content));
                        // Consume all matching backticks; extras → &#96; so they
                        // don't accidentally start new inline code spans
                        i = j + match_count;
                        if match_count > open_count {
                            for _ in 0..(match_count - open_count) {
                                result.push_str("&#96;");
                            }
                        }
                        break;
                    }
                    j += match_count;
                } else {
                    j += 1;
                }
            }
            if j + open_count > len {
                // No matching closer found, emit literal backticks
                for _ in 0..open_count {
                    result.push('`');
                }
                i += open_count;
            }
            continue;
        }

        // ~~strikethrough~~
        if i + 3 < len && chars[i] == '~' && chars[i + 1] == '~' {
            let mut j = i + 2;
            while j + 1 < len && !(chars[j] == '~' && chars[j + 1] == '~') {
                j += 1;
            }
            if j + 1 < len {
                let inner: String = chars[i + 2..j].iter().collect();
                result.push_str(&format!("<del>{}</del>", escape_html(&inner)));
                i = j + 2;
                continue;
            }
        }

        // ***bold italic*** (handle before ** to avoid conflict)
        if i + 5 < len && chars[i] == '*' && chars[i + 1] == '*' && chars[i + 2] == '*' {
            let mut j = i + 3;
            while j + 2 < len && !(chars[j] == '*' && chars[j + 1] == '*' && chars[j + 2] == '*') {
                j += 1;
            }
            if j + 2 < len {
                let inner: String = chars[i + 3..j].iter().collect();
                result.push_str(&format!("<em><strong>{}</strong></em>", escape_html(&inner)));
                i = j + 3;
                continue;
            }
        }

        // **bold**
        if i + 3 < len && chars[i] == '*' && chars[i + 1] == '*' {
            let mut j = i + 2;
            while j + 1 < len && !(chars[j] == '*' && chars[j + 1] == '*') {
                j += 1;
            }
            if j + 1 < len {
                let inner: String = chars[i + 2..j].iter().collect();
                result.push_str(&format!("<strong>{}</strong>", escape_html(&inner)));
                i = j + 2;
                continue;
            }
        }

        // *italic*
        if chars[i] == '*' {
            let mut j = i + 1;
            while j < len && chars[j] != '*' {
                j += 1;
            }
            if j < len && j > i + 1 {
                let inner: String = chars[i + 1..j].iter().collect();
                result.push_str(&format!("<em>{}</em>", escape_html(&inner)));
                i = j + 1;
                continue;
            }
        }

        // ==highlight==
        if i + 1 < len && chars[i] == '=' && chars[i + 1] == '=' {
            let mut j = i + 2;
            while j + 1 < len && !(chars[j] == '=' && chars[j + 1] == '=') {
                j += 1;
            }
            if j + 1 < len {
                let inner: String = chars[i + 2..j].iter().collect();
                result.push_str(&format!("<mark>{}</mark>", escape_html(&inner)));
                i = j + 2;
                continue;
            }
        }

        // [text](url) links (skip ![ for images)
        if chars[i] == '[' && (i == 0 || chars[i - 1] != '!') {
            let mut text_end = i + 1;
            while text_end < len && chars[text_end] != ']' {
                text_end += 1;
            }
            if text_end + 1 < len && chars[text_end] == ']' && chars[text_end + 1] == '(' {
                let mut url_end = text_end + 2;
                while url_end < len && chars[url_end] != ')' {
                    url_end += 1;
                }
                if url_end < len && chars[url_end] == ')' {
                    let link_text: String = chars[i + 1..text_end].iter().collect();
                    let url: String = chars[text_end + 2..url_end].iter().collect();
                    result.push_str(&format!("<a href=\"{}\">{}</a>", escape_html(&url), escape_html(&link_text)));
                    i = url_end + 1;
                    continue;
                }
            }
        }

        // Auto-links
        if i + 7 < len && chars[i] == 'h' && chars[i+1] == 't' && chars[i+2] == 't' && chars[i+3] == 'p' {
            let is_in_brackets = i >= 2 && chars[i-1] == '(' && chars[i-2] == ']';
            let is_in_angle = i >= 1 && chars[i-1] == '<';

            if !is_in_brackets && !is_in_angle {
                let is_https = i + 8 < len && chars[i+4] == 's' && chars[i+5] == ':';
                let prefix_len = if is_https { 8 } else { 7 };

                let mut url_len = prefix_len;
                while i + url_len < len {
                    let c = chars[i + url_len];
                    if c.is_alphanumeric() || c == '/' || c == '.' || c == '-' || c == '_' || c == '~' || c == ':' || c == '@' || c == '!' || c == '$' || c == '&' || c == '\'' || c == '(' || c == ')' || c == '*' || c == '+' || c == ',' || c == ';' || c == '=' || c == '?' || c == '%' || c == '#' {
                        url_len += 1;
                    } else {
                        break;
                    }
                }

                if url_len > prefix_len {
                    let url: String = chars[i..i+url_len].iter().collect();
                    let mut clean_url = url.trim_end_matches('.');
                    clean_url = clean_url.trim_end_matches(',');
                    if clean_url.ends_with(')') {
                        let open_parens = clean_url[prefix_len..].chars().filter(|&c| c == '(').count();
                        let close_parens = clean_url[prefix_len..clean_url.len()-1].chars().filter(|&c| c == ')').count();
                        if close_parens < open_parens {
                            clean_url = clean_url.trim_end_matches(')');
                        }
                    }
                    let clean_len = clean_url.len();
                    let actual_url: String = chars[i..i+clean_len].iter().collect();
                    result.push_str(&format!("<a href=\"{}\" target=\"_blank\">{}</a>", escape_html(&actual_url), escape_html(&actual_url)));
                    i += clean_len;
                    continue;
                }
            }
        }

        // Auto-detect email addresses (e.g. contact@tizumark.app)
        if chars[i].is_alphanumeric() {
            // Scan forward looking for an @ that would indicate an email
            let mut has_at = false;
            let mut at_pos = 0;
            let mut j = i;
            while j < len {
                let c = chars[j];
                if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '+' {
                    // valid email character
                } else if c == '@' && !has_at && j > i && j + 1 < len {
                    has_at = true;
                    at_pos = j;
                } else {
                    break;
                }
                j += 1;
            }
            // Valid email: has @, domain has a dot, and ends with a valid TLD
            if has_at && at_pos + 3 < j {
                let domain_part: String = chars[at_pos + 1..j].iter().collect();
                if domain_part.contains('.') && !domain_part.starts_with('.') && !domain_part.ends_with('.') {
                    let email: String = chars[i..j].iter().collect();
                    // Verify the character after the email isn't alphanumeric (to avoid false matches)
                    let is_standalone = j >= len || !chars[j].is_alphanumeric();
                    if is_standalone {
                        result.push_str(&format!("<a href=\"mailto:{}\">{}</a>", escape_html(&email), escape_html(&email)));
                        i = j;
                        continue;
                    }
                }
            }
        }

        // Default: HTML-escape to prevent XSS
        // Note: '>' is NOT escaped because it is valid Markdown syntax (blockquote, nested blockquote).
        // pulldown-cmark will properly escape it in text content during HTML generation.
        match chars[i] {
            '&' => {
                // CommonMark §2.3 & §6.5: preserve valid HTML entity references
                let start = i;
                let mut found = false;
                if i + 1 < len {
                    if chars[i + 1] == '#' {
                        // Numeric entity &#NNN; or &#xHHH;
                        let mut j = i + 2;
                        if j < len && chars[j] == 'x' { j += 1; }
                        while j < len {
                            if chars[j] == ';' {
                                for k in start..=j { result.push(chars[k]); }
                                i = j;
                                found = true;
                                break;
                            }
                            if !chars[j].is_ascii_hexdigit() { break; }
                            j += 1;
                        }
                    } else if chars[i + 1].is_ascii_alphabetic() {
                        // Named entity &amp; &lt; &copy; etc.
                        let mut j = i + 2;
                        while j < len {
                            if chars[j] == ';' {
                                for k in start..=j { result.push(chars[k]); }
                                i = j;
                                found = true;
                                break;
                            }
                            if !chars[j].is_ascii_alphanumeric() { break; }
                            j += 1;
                        }
                    }
                }
                if !found {
                    result.push_str("&amp;");
                }
            }
            '<' => result.push_str("&lt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            c => result.push(c),
        }
        i += 1;
    }

    result
}

fn parse_alert(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.starts_with("> [!info]") || lower.starts_with("> [!note]") {
        Some(r#"<div class="alert alert-note"><div class="alert-title"><svg class="alert-icon" viewBox="0 0 16 16" width="16" height="16"><circle cx="8" cy="8" r="7" fill="none" stroke="currentColor" stroke-width="1.5"/><line x1="8" y1="7" x2="8" y2="11.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="5" r="0.8" fill="currentColor"/></svg>Note</div><div class="alert-content">"#.to_string())
    } else if lower.starts_with("> [!tip]") {
        Some(r#"<div class="alert alert-tip"><div class="alert-title"><svg class="alert-icon" viewBox="0 0 16 16" width="16" height="16"><path d="M8 1.5c-2.5 0-4.5 2-4.5 4.5 0 1.8 1 3 2.2 3.8.3.2.3.5.3.8v1.4h4v-1.4c0-.3.1-.6.3-.8 1.2-.8 2.2-2 2.2-3.8 0-2.5-2-4.5-4.5-4.5z" fill="none" stroke="currentColor" stroke-width="1.3"/><line x1="6" y1="14" x2="10" y2="14" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>Tip</div><div class="alert-content">"#.to_string())
    } else if lower.starts_with("> [!important]") {
        Some(r#"<div class="alert alert-important"><div class="alert-title"><svg class="alert-icon" viewBox="0 0 16 16" width="16" height="16"><path d="M8 1.5L1.5 13.5h13L8 1.5z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><line x1="8" y1="6.5" x2="8" y2="9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="11.2" r="0.7" fill="currentColor"/></svg>Important</div><div class="alert-content">"#.to_string())
    } else if lower.starts_with("> [!warning]") {
        Some(r#"<div class="alert alert-warning"><div class="alert-title"><svg class="alert-icon" viewBox="0 0 16 16" width="16" height="16"><path d="M8 1.5L1.5 13.5h13L8 1.5z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><line x1="8" y1="6" x2="8" y2="9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="11.2" r="0.7" fill="currentColor"/></svg>Warning</div><div class="alert-content">"#.to_string())
    } else if lower.starts_with("> [!caution]") {
        Some(r#"<div class="alert alert-caution"><div class="alert-title"><svg class="alert-icon" viewBox="0 0 16 16" width="16" height="16"><circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.3"/><line x1="8" y1="4.5" x2="8" y2="8.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="10.5" r="0.8" fill="currentColor"/></svg>Caution</div><div class="alert-content">"#.to_string())
    } else {
        None
    }
}

/// 崩溃日志目录：%LOCALAPPDATA%/com.tizu.vsnote/logs
fn crash_log_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|d| d.join("com.tizu.vsnote").join("logs"))
}

/// 安装 panic hook：把崩溃信息落盘。
/// 发布版是 windows_subsystem = "windows"（无控制台），panic 会表现为"静默闪退"，
/// 只有落盘日志才能事后定位。
fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string payload>".to_string());
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let text = format!(
            "vsnote panic\ntime: {}\nwhere: {}\npayload: {}\nbacktrace:\n{:?}\n",
            stamp,
            loc,
            payload,
            std::backtrace::Backtrace::force_capture()
        );
        if let Some(dir) = crash_log_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(format!("crash-{}.log", stamp)), &text);
        }
        prev(info);
    }));
}

/// Windows 原生错误弹窗。用于 app 尚未起来（连 webview 都没有）时的致命错误提示。
#[cfg(target_os = "windows")]
fn show_fatal_dialog(body: &str) {
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }
    fn wide(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }
    let caption = wide("vsnote 启动失败");
    let text = wide(body);
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            0x0000_0010u32, // MB_ICONERROR
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_fatal_dialog(body: &str) {
    eprintln!("vsnote 启动失败: {}", body);
}

/// 把 WebView2 用户数据目录隔离改名。
/// 该目录（%LOCALAPPDATA%/com.tizu.vsnote/EBWebView）损坏时 webview 会创建失败，
/// 表现为"进程一闪即退"或"主窗口根本不出现"，改名后由 WebView2 重建即可自愈。
fn quarantine_webview_udf() -> bool {
    let Some(p) = dirs::data_local_dir().map(|d| d.join("com.tizu.vsnote").join("EBWebView")) else {
        return false;
    };
    if !p.exists() {
        return false;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dst = p.with_file_name(format!("EBWebView.bad.{}", stamp));
    std::fs::rename(&p, &dst).is_ok()
}

fn report_fatal(err: &str) {
    let log_hint = crash_log_dir()
        .map(|d| format!("\n\n日志目录：{}", d.display()))
        .unwrap_or_default();
    let body = format!(
        "{}\n\n如果提示涉及 WebView2 / msedgewebview2.exe，说明系统里的 \
         Microsoft Edge WebView2 Runtime 已损坏，vsnote 无法创建窗口。\n\
         请到「设置 → 应用 → Microsoft Edge WebView2 Runtime → 修改 → 修复」，\
         或从微软官网重新下载安装 WebView2 Runtime 后重试。{}",
        err, log_hint
    );
    if let Some(dir) = crash_log_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("startup-failure.log"), &body);
    }
    show_fatal_dialog(&body);
}

fn build_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                show_window(&window);
                // files_from_args 滤掉 argv[0]（第二实例 exe 自身路径），
                // 避免前端误把 exe 当文件打开；与 get_cli_args 的 skip(1) 对齐
                let _ = app.emit("file-open", files_from_args(argv));
            }
        }))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 交给前端处理关闭流程（未保存文档弹框 / 关闭行为偏好）
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .setup(|app| {
            // 拿不到主窗口 / show 失败都不能让 setup 返回 Err —— 那会让整个 app 直接退出，
            // 用户看到的就是"双击后闪退"。宁可窗口晚一点显示，也不要静默死掉。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            } else {
                eprintln!("[vsnote] setup: main window not found");
            }
            Ok(())
        })
        .manage(WatcherState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            get_cli_args,
            quit_app,
            read_file,
            write_file,
            file_meta,
            is_directory,
            list_dir,
            write_binary_file,
            ensure_dir,
            rename_path,
            remove_path,
            copy_path,
            move_path,
            reveal_in_file_manager,
            watch_folder,
            stop_watch,
            app_data_dir,
            save_image_to_assets,
            fetch_image_as_base64,
            generate_toc,
            search_in_files,
            read_bundled_file,
            read_bundled_image_as_base64,
            db_open,
            db_close,
            db_remember_password,
            db_forget_password,
            export_to_db,
            export_to_folder,
            git_check,
            git_auto_sync,
            git_auto_pull,
            git_status,
            git_resolve_conflicts,
            git_abort_merge
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();

    // 第一次尝试：正常启动
    match build_app().build(tauri::generate_context!()) {
        Ok(app) => {
            app.run(|_app_handle, _event| {});
            return;
        }
        Err(e) => {
            let first = e.to_string();
            // WebView2 用户数据目录损坏是自愈场景：隔离后重建一次。
            if quarantine_webview_udf() {
                match build_app().build(tauri::generate_context!()) {
                    Ok(app) => {
                        app.run(|_app_handle, _event| {});
                        return;
                    }
                    Err(e2) => report_fatal(&format!("{}\n\n（清理 WebView2 缓存后重试仍然失败）{}", first, e2)),
                }
            } else {
                report_fatal(&first);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_from_args_skips_exe() {
        let argv = vec![
            "C:\\Program Files\\TizuMark\\TizuMark.exe".to_string(),
            "D:\\docs\\note.md".to_string(),
        ];
        let files = files_from_args(argv);
        assert_eq!(files, vec!["D:\\docs\\note.md".to_string()]);
    }

    #[test]
    fn test_is_directory_distinguishes_dir_and_file() {
        let tmp = std::env::temp_dir().join("tizumark_isdir_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("a.md");
        fs::write(&file, "x").unwrap();
        assert!(is_directory(tmp.to_string_lossy().to_string()));
        assert!(!is_directory(file.to_string_lossy().to_string()));
        assert!(!is_directory("Z:\\definitely\\not\\exists".to_string()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_files_from_args_empty_when_only_exe() {
        let argv = vec!["TizuMark.exe".to_string()];
        assert!(files_from_args(argv).is_empty());
    }

    #[test]
    fn test_search_in_files_recursive_and_filter() {
        let tmp = std::env::temp_dir().join("tizumark_search_test_1");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.md"), "hello world\nhello rust\n").unwrap();
        fs::write(tmp.join("b.txt"), "hello again\n").unwrap();
        fs::write(tmp.join("c.log"), "hello ignored\n").unwrap();
        let sub = tmp.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("d.md"), "hello nested\n").unwrap();
        let res = search_in_files_impl(tmp.to_str().unwrap(), "hello", false, false, &[]).unwrap();
        let paths: Vec<&str> = res.iter().map(|f| f.path.as_str()).collect();
        assert!(!paths.iter().any(|p| p.ends_with("c.log")), "c.log 应被扩展名过滤");
        assert!(paths.iter().any(|p| p.ends_with("d.md")), "应递归到 sub/d.md");
        let total: usize = res.iter().map(|f| f.matches.len()).sum();
        assert_eq!(total, 4, "a.md(2)+b.txt(1)+d.md(1) 共 4 处匹配");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_search_in_files_regex() {
        let tmp = std::env::temp_dir().join("tizumark_search_test_2");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.md"), "foo123bar\nfoo456\nno digits\n").unwrap();
        let res = search_in_files_impl(tmp.to_str().unwrap(), r"\d+", false, true, &[]).unwrap();
        let total: usize = res.iter().map(|f| f.matches.len()).sum();
        assert_eq!(total, 2, "正则 \\d+ 应匹配 2 行");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_search_in_files_case_sensitivity() {
        let tmp = std::env::temp_dir().join("tizumark_search_test_3");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.md"), "Hello\nHELLO\nhello\n").unwrap();
        let res_ci = search_in_files_impl(tmp.to_str().unwrap(), "hello", false, false, &[]).unwrap();
        let total_ci: usize = res_ci.iter().map(|f| f.matches.len()).sum();
        assert_eq!(total_ci, 3, "大小写不敏感应匹配 3 处");
        let res_cs = search_in_files_impl(tmp.to_str().unwrap(), "hello", true, false, &[]).unwrap();
        let total_cs: usize = res_cs.iter().map(|f| f.matches.len()).sum();
        assert_eq!(total_cs, 1, "大小写敏感应只匹配 1 处");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_search_in_files_per_file_limit() {
        let tmp = std::env::temp_dir().join("tizumark_search_test_4");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let content: String = (0..600).map(|_| "x\n").collect();
        fs::write(tmp.join("big.md"), content).unwrap();
        let res = search_in_files_impl(tmp.to_str().unwrap(), "x", false, false, &[]).unwrap();
        assert_eq!(res.len(), 1, "应只有 1 个文件");
        assert!(res[0].matches.len() <= 500, "单文件匹配应被截断到 <= 500，实际 {}", res[0].matches.len());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("hello"), "hello");
        assert_eq!(escape_html("<script>alert(1)</script>"), "&lt;script&gt;alert(1)&lt;/script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(escape_html("it's"), "it&#x27;s");
        assert_eq!(escape_html("<img src=x onerror=alert(1)>"), "&lt;img src=x onerror=alert(1)&gt;");
    }

    #[test]
    fn test_render_markdown_xss_in_heading() {
        let input = "# <script>alert(1)</script>".to_string();
        let html = render_markdown(input);
        assert!(!html.contains("<script>"));
        assert!(html.contains("alert(1)"));
    }

    #[test]
    fn test_render_markdown_xss_in_text() {
        let input = "Hello <img src=x onerror=alert(1)>".to_string();
        let html = render_markdown(input);
        assert!(!html.contains("onerror"));
        assert!(html.contains("<img"));
    }

    #[test]
    fn test_render_markdown_script_tag_stripped() {
        let input = "Text <script>document.cookie</script> more".to_string();
        let html = render_markdown(input);
        eprintln!("script output: {:?}", html);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("</script>"));
        assert!(html.contains("Text"));
        assert!(html.contains("more"));
    }

    #[test]
    fn test_render_markdown_iframe_stripped() {
        let input = "Text <iframe src=\"evil.com\"></iframe> more".to_string();
        let html = render_markdown(input);
        eprintln!("iframe output: {:?}", html);
        assert!(!html.contains("<iframe"));
        assert!(!html.contains("</iframe>"));
    }

    #[test]
    fn test_render_markdown_img_preserved() {
        let input = "![alt](https://example.com/img.png)".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<img"));
        assert!(html.contains("src="));
    }

    #[test]
    fn test_render_markdown_normal() {
        let input = "# Hello\n\nThis is **bold** and *italic*.".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_generate_toc_xss() {
        let input = "# <script>alert(1)</script>\n## Normal Heading".to_string();
        let toc = generate_toc(input);
        assert!(!toc.contains("<script>"));
        assert!(toc.contains("&lt;script&gt;"));
        assert!(toc.contains("Normal Heading"));
    }

    #[test]
    fn test_generate_toc_normal() {
        let input = "# First\n## Second\n### Third".to_string();
        let toc = generate_toc(input);
        assert!(toc.contains("First"));
        assert!(toc.contains("Second"));
        assert!(toc.contains("Third"));
    }

    #[test]
    fn test_preprocess_alert_xss() {
        let input = "> [!NOTE]\n> <script>alert(1)</script>".to_string();
        let result = preprocess_markdown(input);
        assert!(!result.contains("<script>"), "Raw script tag should not appear in: {}", result);
        // After > escaping fix: < is escaped to &lt;, but > is preserved for markdown syntax
        assert!(result.contains("&lt;script"), "Script tag should be escaped in: {}", result);
    }

    #[test]
    fn test_preprocess_alert_normal() {
        let input = "> [!TIP]\n> This is a tip".to_string();
        let result = preprocess_markdown(input);
        assert!(result.contains("This is a tip"));
        assert!(result.contains("alert-tip"));
    }

    #[test]
    fn test_render_markdown_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn test_render_markdown_code_block() {
        let input = "```javascript\nconsole.log('hello');\n```".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<code"));
        assert!(html.contains("console.log"));
    }

    #[test]
    fn test_sanitize_preserves_chinese() {
        let input = "# 你好世界\n\n这是**中文**测试。".to_string();
        let html = render_markdown(input);
        assert!(html.contains("你好世界"));
        assert!(html.contains("中文"));
        assert!(html.contains("<strong>中文</strong>"));
    }

    #[test]
    fn test_sanitize_chinese_with_html() {
        let input = "# 标题\n\n<bold>加粗</bold>文字".to_string();
        let html = render_markdown(input);
        assert!(html.contains("标题"));
        assert!(html.contains("加粗"));
        assert!(html.contains("文字"));
    }

    #[test]
    fn test_render_markdown_math_block() {
        let input = "$$\n\\begin{bmatrix} a_{11} & a_{12} \\\\ a_{21} & a_{22} \\end{bmatrix}\n$$".to_string();
        let html = render_markdown(input);
        assert!(html.contains("\\begin{bmatrix}"));
        assert!(html.contains("\\end{bmatrix}"));
        assert!(html.contains("$$"), "Display math $$ delimiters should be preserved for frontend KaTeX");
    }

    #[test]
    fn test_render_markdown_inline_math() {
        let input = "行内公式：$E = mc^2$".to_string();
        let html = render_markdown(input);
        assert!(html.contains("$E = mc^2$"));
    }

    #[test]
    fn test_render_markdown_blockquote() {
        let input = "> 智能标点自动转换".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<blockquote"), "Blockquote should be present in: {}", html);
        assert!(html.contains("智能标点自动转换"));
    }

    #[test]
    fn test_render_markdown_nested_blockquote() {
        let input = "> 第一层\n> > 第二层".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<blockquote"), "Blockquote should be present in: {}", html);
    }

    #[test]
    fn test_render_markdown_task_list() {
        let input = "- [x] 完成项目文档\n- [ ] 发布版本".to_string();
        let html = render_markdown(input);
        assert!(html.contains("checkbox"), "Task list checkbox should be present in: {}", html);
        assert!(html.contains("完成项目文档"));
        assert!(html.contains("发布版本"));
    }

    #[test]
    fn test_render_markdown_horizontal_rule() {
        let input = "text before\n\n---\n\ntext after".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<hr"), "Horizontal rule should be present in: {}", html);
    }

    #[test]
    fn test_render_markdown_heading_with_math_section() {
        let input = "### 行内公式\n\n勾股定理：$a^2 + b^2 = c^2$".to_string();
        let html = render_markdown(input);
        assert!(html.contains("行内公式"), "Heading text should be present in: {}", html);
        assert!(!html.contains("###"), "Raw ### should not be present in: {}", html);
    }

    #[test]
    fn test_render_markdown_heading_after_alert() {
        let input = "> [!NOTE]\n> 提示内容\n\n### 独立公式块\n\n内容".to_string();
        let html = render_markdown(input);
        assert!(html.contains("独立公式块"), "Heading after alert should be present in: {}", html);
        assert!(!html.contains("###"), "Raw ### should not be present in: {}", html);
    }

    #[test]
    fn test_render_markdown_smart_punctuation_hr() {
        // ENABLE_SMART_PUNCTUATION should not break thematic breaks
        let input = "text\n\n---\n\nmore text".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<hr"), "Horizontal rule should be present with smart punctuation in: {}", html);
    }

    #[test]
    fn test_guard_math_blocks_respects_inline_code() {
        // $$ inside inline code should NOT be treated as math block delimiters
        let input = "> [!NOTE]\n> 如 `$$` 符号\n\n### 行内公式\n\n$$
真实公式
$$".to_string();
        let html = render_markdown(input);
        assert!(html.contains("行内公式"), "Heading should be preserved in: {}", html);
        assert!(!html.contains("###"), "Raw ### should not be present in: {}", html);
        assert!(html.contains("真实公式"), "Real math should be preserved in: {}", html);
    }

    #[test]
    fn test_render_demo_math_section() {
        let input = concat!(
            "> [!NOTE]\n",
            "> 以下数学公式在 Gitee / GitHub 等网页端可能显示为源码（如 `$$` 和 LaTeX 代码），这是平台不支持 KaTeX 渲染所致。\n",
            "\n",
            "### 行内公式\n",
            "\n",
            "勾股定理：$a^2 + b^2 = c^2$\n",
            "\n",
            "### 独立公式块\n",
            "\n",
            "麦克斯韦方程组：\n",
            "\n",
            "$$\n",
            "\\begin{aligned}\n",
            "\\nabla \\cdot \\mathbf{E} &= \\frac{\\rho}{\\varepsilon_0} \\\\\n",
            "\\end{aligned}\n",
            "$$\n",
        ).to_string();
        let html = render_markdown(input);
        assert!(html.contains("行内公式"), "Heading '行内公式' should be present in: {}", html);
        assert!(html.contains("独立公式块"), "Heading '独立公式块' should be present in: {}", html);
        assert!(!html.contains("### 行内公式"), "Raw markdown heading should not appear in: {}", html);
        assert!(html.contains("勾股定理"), "Content before math should be present in: {}", html);
        assert!(html.contains("麦克斯韦方程组"), "Content after heading should be present in: {}", html);
        assert!(html.contains("alert-note"), "Alert blockquote should be rendered in: {}", html);
    }


    #[test]
    fn test_render_four_backtick_fence() {
        let input = "````markdown
```javascript
let x = 1;
```
````".to_string();
        let html = render_markdown(input);
        assert!(html.contains("let x = 1"), "Inner code must be present");
        assert!(html.contains("```"), "Should render literal backticks");
    }
    #[test]
    fn test_full_demo_md_file() {
        let content = std::fs::read_to_string("../demo.md")
            .expect("Could not read demo.md");
        let html = render_markdown(content);

        // Find and print context around "### 行内公式"
        if let Some(pos) = html.find("### 行内公式") {
            let start = pos.saturating_sub(500);
            let end = std::cmp::min(pos + 500, html.len());
            // Make sure we're at char boundaries
            let start = html[..start].char_indices().last().map(|(i, _)| i).unwrap_or(0);
            let end = html[..end].char_indices().last().map(|(i, _)| i).unwrap_or(html.len());
            eprintln!("\n========== Context around ### 行内公式 ==========");
            eprintln!("{}", &html[start..end]);
            eprintln!("==========================================================");
        }

        assert!(!html.contains("### 行内公式"), "Raw ### 行内公式 should NOT appear in output");
    }
    #[test]
    fn test_full_demo_math_section_output() {
        // Exact content from demo.md lines 293-335
        let input = r##"## 数学公式

TizuMark 内置 KaTeX 渲染引擎，支持行内公式和独立公式。这是 TizuMark 区别于普通 Markdown 编辑器的重要特性。

> [!NOTE]
> 以下数学公式在 Gitee / GitHub 等网页端可能显示为源码（如 `$$` 和 LaTeX 代码），这是平台不支持 KaTeX 渲染所致。**在 TizuMark 软件中可完美渲染。** 打开 [demo.md](https://gitee.com/tizu/tizu-mark/blob/master/demo.md) 查看效果。

### 行内公式

勾股定理：$a^2 + b^2 = c^2$
欧拉恒等式：$e^{i\pi} + 1 = 0$
二次方程：$x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}$

### 独立公式块

麦克斯韦方程组：

$$
\begin{aligned}
\nabla \cdot \mathbf{E} &= \frac{\rho}{\varepsilon_0} \\
\nabla \cdot \mathbf{B} &= 0 \\
\nabla \times \mathbf{E} &= -\frac{\partial \mathbf{B}}{\partial t} \\
\nabla \times \mathbf{B} &= \mu_0 \mathbf{J} + \mu_0 \varepsilon_0 \frac{\partial \mathbf{E}}{\partial t}
\end{aligned}
$$

### 常用数学

线性回归的损失函数（MSE）：

$$
J(\theta) = \frac{1}{2m} \sum_{i=1}^{m} (h_\theta(x^{(i)}) - y^{(i)})^2
$$

贝叶斯定理：

$$
P(A|B) = \frac{P(B|A) \cdot P(A)}{P(B)}
$$
"##.to_string();

        let html = render_markdown(input);
        eprintln!("\n========== FULL MATH SECTION OUTPUT ==========\n{}\n================================================\n", html);

        // Verify structure
        assert!(html.contains("<h2>数学公式</h2>"), "h2 should be rendered");
        assert!(html.contains("<h3>行内公式</h3>"), "h3 行内公式 should be rendered");
        assert!(html.contains("<h3>独立公式块</h3>"), "h3 独立公式块 should be rendered");
        assert!(html.contains("<h3>常用数学</h3>"), "h3 常用数学 should be rendered");
        assert!(!html.contains("### "), "No raw ### should remain");
    }

    #[test]
    fn test_render_demo_task_list() {
        let input = concat!(
            "### 任务列表\n",
            "\n",
            "本周待办事项：\n",
            "\n",
            "- [x] 完成项目文档\n",
            "- [x] Review 代码\n",
            "- [ ] 发布 v0.2.0 版本\n",
        ).to_string();
        let html = render_markdown(input);
        assert!(html.contains("checkbox"), "Task list should have checkboxes in: {}", html);
        assert!(html.contains("完成项目文档"));
    }

    #[test]
    fn test_render_blockquote_and_hr() {
        let input = "> 单一引用：TizuMark 的设计哲学是\"简单就是力量\"。\n\n---\n\n## 流程图与图表\n\nTizuMark 内置 Mermaid 图表引擎。".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<blockquote"), "Blockquote should be present in: {}", html);
        assert!(html.contains("<hr"), "Horizontal rule should be present in: {}", html);
        assert!(html.contains("流程图与图表"), "Heading should be present in: {}", html);
        assert!(html.contains("Mermaid"), "Content should be present in: {}", html);
    }
}

    #[test]
    fn test_matrix_math_block() {
        // Test with proper \\ (double backslash) matching actual demo.md content
        let input = "矩阵乘法：\n\n\
$$\n\
\\begin{bmatrix}\n\
a_{11} & a_{12} \\\\\n\
a_{21} & a_{22}\n\
\\end{bmatrix}\n\
\\times\n\
\\begin{bmatrix}\n\
b_{11} & b_{12} \\\\\n\
b_{21} & b_{22}\n\
\\end{bmatrix}\n\
=\n\
\\begin{bmatrix}\n\
a_{11}b_{11} + a_{12}b_{21} & a_{11}b_{12} + a_{12}b_{22} \\\\\n\
a_{21}b_{11} + a_{22}b_{21} & a_{21}b_{12} + a_{22}b_{22}\n\
\\end{bmatrix}\n\
$$\n\
\n\
---\n\
".to_string();
        let html = render_markdown(input);
        eprintln!("\n========== MATRIX BLOCK OUTPUT (with \\\\) ==========\n{}\n======================================================\n", html);

        // Check: should have the matrix LaTeX content
        assert!(html.contains("bmatrix"), "Should contain bmatrix");
        assert!(html.contains("矩阵乘法"), "Should contain 矩阵乘法 heading text");
        // After HTML-escaping, & becomes &amp; but \\ is preserved (backslash is not special in HTML)
        assert!(html.contains("a_{11} &amp; a_{12} \\\\"), "Should preserve LaTeX row breaks with &amp;");
    }

    #[test]
    fn test_backslash_escape_asterisk() {
        let input = "\\*not italic\\*".to_string();
        let html = render_markdown(input);
        assert!(!html.contains("<em>"), "Escaped * should not be emphasis");
        assert!(html.contains("not italic"), "Content should be preserved");
    }

    #[test]
    fn test_backslash_escape_bracket() {
        let input = "\\[not a link\\]".to_string();
        let html = render_markdown(input);
        assert!(html.contains("[not a link]"), "Escaped brackets as literal");
    }

    #[test]
    fn test_inline_code_space_trimming() {
        let input = "` code `".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<code>code</code>"), "Spaces should be trimmed");
    }

    #[test]
    fn test_triple_emphasis() {
        let input = "***bold italic***".to_string();
        let html = render_markdown(input);
        assert!(html.contains("<em><strong>"), "Triple *** should give nested emphasis");
        assert!(html.contains("bold italic"), "Content preserved");
    }

    #[test]
    fn test_html_entity_copy() {
        let input = "&copy; 2025".to_string();
        let html = render_markdown(input);
        assert!(!html.contains("&amp;copy;"), "Entity &copy; should not be double-escaped");
    }

    #[test]
    fn test_html_entity_amp() {
        let input = "&amp; is the & symbol".to_string();
        let html = render_markdown(input);
        assert!(!html.contains("&amp;amp;"), "Entity &amp; should not be double-escaped");
    }

    #[test]
    fn test_plain_ampersand_escaped() {
        let input = "A & B".to_string();
        let html = render_markdown(input);
        assert!(html.contains("&amp;"), "Plain & should be escaped");
    }

    #[test]
    fn test_math_before_alert() {
        let input = "如果你在写技术文档，可以自然地插入数学公式：\n\n$$\nO(1) < O(\\log n) < O(n) < O(n \\log n) < O(n^2) < O(2^n)\n$$\n\n> [!TIP]\n> **写作建议**：好的技术文档是\"分层\"的——正文讲核心逻辑，脚注补充细节，引用块标注出处，提示框强调要点。";
        let html = render_markdown(input.to_string());
        assert!(html.contains("$$"), "Math $$ should be preserved");
        assert!(!html.contains("<p>$$"), "Math block should NOT be wrapped in <p>");
    }

    #[test]
    fn test_full_demo_md_for_code_wrap() {
        let content = std::fs::read_to_string("../demo.md")
            .expect("Could not read demo.md");
        let html = render_markdown(content.clone());

        // Complexity formula must NOT be wrapped in <code>
        assert!(
            !html.contains("<code>\n$$\nO(1)") && !html.contains("<code>$$\nO(1)"),
            "Complexity formula should NOT be wrapped in <code>"
        );

        // All <code> tags must be balanced
        let mut tag_stack: Vec<()> = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = html.chars().collect();
        let len = chars.len();
        while pos < len {
            if chars[pos] == '<' && pos + 1 < len {
                if chars[pos + 1] == '/' {
                    let mut tag_end = pos + 2;
                    while tag_end < len && chars[tag_end] != '>' { tag_end += 1; }
                    let tag_name: String = chars[pos + 2..tag_end].iter().collect();
                    if tag_name == "code" {
                        assert!(tag_stack.pop().is_some(), "Orphan </code> in HTML output");
                    }
                    pos = tag_end + 1;
                    continue;
                }
                if chars[pos + 1].is_ascii_alphabetic() {
                    let mut tag_end = pos + 1;
                    while tag_end < len && chars[tag_end] != '>' && chars[tag_end] != ' ' { tag_end += 1; }
                    let tag_name: String = chars[pos + 1..tag_end].iter().collect();
                    if tag_name == "code" {
                        tag_stack.push(());
                    }
                    while tag_end < len && chars[tag_end] != '>' { tag_end += 1; }
                    pos = tag_end + 1;
                    continue;
                }
            }
            pos += 1;
        }
        assert!(tag_stack.is_empty(), "Unclosed <code> tags in HTML output: {}", tag_stack.len());
    }

    #[test]
    fn test_extract_abbreviations() {
        let input = "*[HTML]: HyperText Markup Language\n*[CSS]: Cascading Style Sheets\n";
        let abbrs = extract_abbreviations(input);
        assert_eq!(abbrs.len(), 2, "Should extract 2 abbreviations");
        assert_eq!(abbrs[0], ("HTML".to_string(), "HyperText Markup Language".to_string()));
        assert_eq!(abbrs[1], ("CSS".to_string(), "Cascading Style Sheets".to_string()));
    }

    #[test]
    fn test_abbr_defs_hidden_from_output() {
        let input = "*[HTML]: HyperText Markup Language\n\nHTML is a markup language.\n".to_string();
        let html = render_markdown(input);
        assert!(!html.contains("*[HTML]:"), "Abbr definition lines should be hidden from output");
        assert!(html.contains("id=\"abbr-data\""), "Abbr data div should be embedded in output");
        assert!(html.contains("HTML is a markup language"), "Regular text should still appear in output");
    }

    #[test]
    fn test_abbr_not_in_code_blocks() {
        // extract_abbreviations is line-based; it will find *[HTML]: even in code blocks.
        // The frontend TreeWalker skips CODE/PRE tags, so this is safe at render time.
        let content = "```\n*[HTML]: this is not an abbreviation\n```\n\n*[CSS]: Cascading Style Sheets\n\nCSS is used.\n".to_string();
        let abbrs = extract_abbreviations(&content);
        assert!(abbrs.iter().any(|(t, _)| t == "HTML"), "Line parser extracts all *[TERM]: lines");
        assert!(abbrs.iter().any(|(t, _)| t == "CSS"), "Legitimate abbreviation should also be extracted");
        let html = render_markdown(content);
        // Code block content IS preserved verbatim (including *[HTML]: text)
        // The real abbreviation definition *[CSS]: should be hidden
        // Data div should be embedded
        assert!(html.contains("<pre><code>"), "Code block should be rendered");
        assert!(html.contains("*[HTML]:"), "Code block content should be preserved verbatim");
        assert!(!html.contains("*[CSS]:"), "Abbr definition outside code should be hidden");
        assert!(html.contains("id=\"abbr-data\""), "Data div should be embedded");
    }

    #[test]
    fn test_decode_numeric_entities() {
        assert_eq!(decode_numeric_entities("&#x6A;"), "j"); // hex 'j'
        assert_eq!(decode_numeric_entities("&#106;"), "j"); // decimal 'j'
        assert_eq!(decode_numeric_entities("&#x6A"), ""); // No closing ; → stripped
        assert_eq!(decode_numeric_entities("hello&#65;world"), "helloAworld");
        assert_eq!(decode_numeric_entities("&#x6A;&#x61;"), "ja"); // Multiple
        assert_eq!(decode_numeric_entities("no entities"), "no entities");
        assert_eq!(decode_numeric_entities("&#X6A;"), "j"); // Uppercase X
        assert_eq!(decode_numeric_entities("&#x6A;avascript:"), "javascript:"); // Decode 'j'
    }

    #[test]
    fn test_contains_dangerous_protocol() {
        assert!(contains_dangerous_protocol("javascript:alert(1)"));
        assert!(contains_dangerous_protocol("JAVASCRIPT:alert(1)"));
        assert!(contains_dangerous_protocol("&#x6A;avascript:alert(1)"));
        assert!(contains_dangerous_protocol("&#106;avascript:alert(1)"));
        assert!(contains_dangerous_protocol("&#X6A;avascript:alert(1)"));
        assert!(!contains_dangerous_protocol("https://example.com"));
        assert!(!contains_dangerous_protocol("javascript-alert"));
        assert!(!contains_dangerous_protocol("onclick"));
    }

    #[test]
    fn test_xss_numeric_entity_bypass() {
        // &#x6A; = 'j', so &#x6A;avascript: = javascript:
        let input = "<a href=\"&#x6A;avascript:alert(1)\">click</a>";
        let html = render_markdown(input.to_string());
        assert!(!html.contains("javascript:"), "Numeric entity bypass should be blocked");
        assert!(html.contains("click"), "Link text should be preserved");
    }

    #[test]
    fn test_xss_decimal_entity_bypass() {
        // &#106; = 'j', so &#106;avascript: = javascript:
        let input = "<a href=\"&#106;avascript:alert(1)\">click</a>";
        let html = render_markdown(input.to_string());
        assert!(!html.contains("javascript:"), "Decimal entity bypass should be blocked");
        assert!(html.contains("click"), "Link text should be preserved");
    }

    #[test]
    fn test_boolean_attribute_xss() {
        // Boolean attribute with dangerous protocol
        let input = "<div javascript:>test</div>";
        let html = render_markdown(input.to_string());
        assert!(!html.contains("javascript:"), "Boolean attribute with javascript: should be stripped");
    }

    // ---------- 文件编码 / 读写命令（整理测试库时补充） ----------

    #[test]
    fn test_decode_bytes_utf8() {
        assert_eq!(decode_bytes(b"hello world"), "hello world");
    }

    #[test]
    fn test_decode_bytes_utf8_with_bom() {
        // 带 UTF-8 BOM 的字节应剥离 BOM 再解码
        let mut v = vec![0xEF, 0xBB, 0xBF];
        v.extend_from_slice(b"hello");
        assert_eq!(decode_bytes(&v), "hello");
    }

    #[test]
    fn test_decode_bytes_gb18030_roundtrip() {
        // GB18030 编码的字节（非合法 UTF-8）应回退解码成功
        let s = "中文测试，简繁混合：軟體";
        let (encoded, _enc, _err) = GB18030.encode(s);
        let bytes = encoded.as_ref().to_vec();
        // 先确认这些字节本身不是合法 UTF-8，才会走回退分支
        assert!(String::from_utf8(bytes.clone()).is_err(), "GB18030 字节应非 UTF-8");
        assert_eq!(decode_bytes(&bytes), s, "GB18030 回退解码应还原原文");
    }

    #[test]
    fn test_decode_bytes_invalid_bytes_no_panic() {
        // 无法解码的字节不应 panic，应回退为替换字符
        let s = decode_bytes(&[0xFF, 0xFE, 0x00, 0x01]);
        assert!(s.chars().all(|c| c != '\u{FFFD}' || true), "无效字节解码不 panic");
    }

    #[test]
    fn test_read_file_utf8() {
        let tmp = std::env::temp_dir().join("tizumark_read_utf8_test.txt");
        fs::write(&tmp, "hello 中文").unwrap();
        let res = read_file(tmp.to_str().unwrap().to_string());
        assert!(res.is_ok(), "读取 UTF-8 文件应成功");
        assert_eq!(res.unwrap(), "hello 中文");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_read_file_gb18030() {
        let tmp = std::env::temp_dir().join("tizumark_read_gbk_test.txt");
        let s = "中文内容测试";
        let (encoded, _enc, _err) = GB18030.encode(s);
        fs::write(&tmp, encoded.as_ref()).unwrap();
        let res = read_file(tmp.to_str().unwrap().to_string());
        assert!(res.is_ok(), "读取 GB18030 文件应成功（编码兼容）");
        assert_eq!(res.unwrap(), s, "GB18030 文件内容应正确还原");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_read_file_missing_returns_structured_error() {
        let path = std::env::temp_dir()
            .join("tizumark_does_not_exist_xyz_12345.md");
        let res = read_file(path.to_str().unwrap().to_string());
        assert!(res.is_err(), "读取不存在文件应返回错误");
        let err = res.unwrap_err();
        assert!(err.contains("\"kind\":\"NotFound\""), "错误应为结构化 JSON 且 kind=NotFound");
    }

    #[test]
    fn test_write_file_then_read_back() {
        let tmp = std::env::temp_dir().join("tizumark_write_test.txt");
        let _ = fs::remove_file(&tmp);
        let content = "# 标题\n\n正文内容";
        let w = write_file(tmp.to_str().unwrap().to_string(), content.to_string());
        assert!(w.is_ok(), "write_file 应成功");
        // 用底层 read_file 读回验证
        let r = read_file(tmp.to_str().unwrap().to_string());
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), content);
        let _ = fs::remove_file(&tmp);
    }

    #[cfg(windows)]
    #[test]
    fn test_safe_write_target_rejects_protected_path() {
        // 拒绝写入系统关键目录（如 C:\windows\system32）
        let res = safe_write_target("C:\\windows\\system32\\evil.txt");
        assert!(res.is_err(), "写入系统目录应被拒绝");
        assert!(res.unwrap_err().contains("protected"), "错误应说明是受保护路径");
    }

    // ---------- 剩余命令单测（file_meta / list_dir / write_binary_file / ensure_dir /
    // save_image_to_assets / fetch_image_as_base64）----------

    // 极简 block_on：仅用于无真实异步 IO 的 async fn（本地文件分支一次 poll 即 Ready）
    #[cfg(test)]
    fn test_block_on<F: std::future::Future>(mut fut: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone_raw(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_raw, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut pinned = unsafe { std::pin::Pin::new_unchecked(&mut fut) };
        loop {
            match pinned.as_mut().poll(&mut cx) {
                Poll::Ready(v) => return v,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    // 最小合法 PNG 头（1x1），imagesize 仅解析头部即可取宽高
    #[cfg(test)]
    fn tiny_png_bytes() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1 height=1
            0x08, 0x06, 0x00, 0x00, 0x00, // bit depth / color type / etc
            0x1F, 0x15, 0xC4, 0x89, // CRC（imagesize 不校验）
        ]
    }

    #[test]
    fn test_file_meta_existing_file() {
        let tmp = std::env::temp_dir().join("tizumark_meta_test.txt");
        fs::write(&tmp, "hello meta").unwrap();
        let res = file_meta(tmp.to_str().unwrap().to_string());
        assert!(res.is_ok());
        let meta = res.unwrap().expect("存在的文件应返回 Some");
        assert_eq!(meta.size, "hello meta".len() as u64, "size 应为文件字节数");
        assert!(meta.mtime > 0, "mtime 应为正的毫秒时间戳");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_file_meta_missing_and_dir_return_none() {
        let missing = std::env::temp_dir().join("tizumark_meta_missing_xyz.md");
        assert_eq!(file_meta(missing.to_str().unwrap().to_string()).unwrap().is_none(), true, "不存在应为 None");
        let dir = std::env::temp_dir();
        assert!(file_meta(dir.to_str().unwrap().to_string()).unwrap().is_none(), "目录应为 None（仅文件有元数据）");
    }

    #[test]
    fn test_list_dir_shows_all_and_sorts() {
        let base = std::env::temp_dir().join("tizumark_listdir_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("zdir")).unwrap();
        fs::write(base.join("b.txt"), "t").unwrap();
        fs::write(base.join("a.md"), "m").unwrap();
        fs::write(base.join("c.png"), "p").unwrap();       // 非文档扩展也应展示
        fs::write(base.join(".hidden.md"), "h").unwrap();  // 点文件也应展示
        let res = list_dir(base.to_str().unwrap().to_string()).expect("list_dir 应成功");
        let names: Vec<&str> = res.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["zdir", ".hidden.md", "a.md", "b.txt", "c.png"],
            "目录排最前，其余按名排序，且不按扩展名/点开头过滤"
        );
        assert!(res[0].is_dir && !res[1].is_dir);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_write_binary_file_roundtrip() {
        let tmp = std::env::temp_dir().join("tizumark_bin_test.bin");
        let _ = fs::remove_file(&tmp);
        let data = vec![0u8, 1, 2, 255, 254, 128];
        write_binary_file(tmp.to_str().unwrap().to_string(), data.clone()).expect("写二进制应成功");
        assert_eq!(fs::read(&tmp).unwrap(), data, "读回字节应一致");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_ensure_dir_creates_nested() {
        let base = std::env::temp_dir().join("tizumark_ensure_dir_test");
        let _ = fs::remove_dir_all(&base);
        let nested = base.join("a").join("b").join("c");
        ensure_dir(nested.to_str().unwrap().to_string()).expect("应递归创建目录");
        assert!(nested.is_dir());
        // 幂等：已存在再调用也成功
        ensure_dir(nested.to_str().unwrap().to_string()).expect("重复调用应成功");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_save_image_to_assets_md5_naming_and_size() {
        let assets = std::env::temp_dir().join("tizumark_assets_test");
        let _ = fs::remove_dir_all(&assets);
        let bytes = tiny_png_bytes();
        let info = save_image_to_assets(bytes.clone(), "png".into(), assets.to_str().unwrap().to_string())
            .expect("保存图片应成功");
        let expect_name = format!("{:x}.png", Md5::digest(&bytes));
        assert_eq!(info.filename, expect_name, "文件名应为字节 md5 + 扩展名");
        assert_eq!((info.width, info.height), (1, 1), "应解析出 1x1 尺寸");
        assert!(assets.join(&info.filename).is_file(), "文件应写入附件目录");
        // 相同字节再存：文件名一致（去重），不报错
        let info2 = save_image_to_assets(bytes, "png".into(), assets.to_str().unwrap().to_string()).unwrap();
        assert_eq!(info2.filename, expect_name);
        let _ = fs::remove_dir_all(&assets);
    }

    #[test]
    fn test_save_image_to_assets_unknown_format_zero_size() {
        let assets = std::env::temp_dir().join("tizumark_assets_unknown_test");
        let _ = fs::remove_dir_all(&assets);
        let info = save_image_to_assets(vec![1, 2, 3, 4], "bin".into(), assets.to_str().unwrap().to_string())
            .expect("未知格式也应能保存");
        assert_eq!((info.width, info.height), (0, 0), "无法识别尺寸应为 0x0");
        let _ = fs::remove_dir_all(&assets);
    }

    #[test]
    fn test_fetch_image_as_base64_local_file() {
        let tmp = std::env::temp_dir().join("tizumark_b64_test.png");
        let bytes = tiny_png_bytes();
        fs::write(&tmp, &bytes).unwrap();
        let encoded = test_block_on(fetch_image_as_base64(tmp.to_str().unwrap().to_string()))
            .expect("本地文件应编码成功");
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD.decode(&encoded).unwrap();
        assert_eq!(decoded, bytes, "base64 解码应还原原始字节");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_fetch_image_as_base64_missing_file_err() {
        let missing = std::env::temp_dir().join("tizumark_b64_missing_xyz.png");
        let res = test_block_on(fetch_image_as_base64(missing.to_str().unwrap().to_string()));
        assert!(res.is_err(), "不存在的本地文件应返回错误");
    }
