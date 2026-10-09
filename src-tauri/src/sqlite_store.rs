// SQLite 工作区存储层（sqlite_store）
//
// 功能：允许把一个 SQLite db 文件作为「根文件夹」打开。db 内的 node 表即文件树，
// node_content 存 md/markdown/txt 文本，node_blob 存图片等二进制（粘贴图片等场景）。
//
// 路由模型（虚拟路径）：
//   - 打开 db 后以「db 文件的真实绝对路径」作为工作区根注册进 registry；
//   - 子节点虚拟路径 = 根路径 + '/' + 相对路径（如 `C:\notes\book.db/日记/2026.md`）；
//   - 前端路径处理（joinPath/parentPath/baseName 等）按 '/' 拆分即可工作，
//     lib.rs 中的 list_dir / read_file / write_file / rename_path / ... 在入口
//     通过 match_root 判定归属后转调本模块，前端无感知。
//
// 加密（可设置密码）：
//   - 纯 Rust 实现（RustCrypto），无 SQLCipher/OpenSSL C 依赖，三平台 CI 零风险；
//   - 密码为空：db 是完全标准的 SQLite 文件（可用任意工具打开）；
//   - 设密码：PBKDF2-HMAC-SHA256(600k 轮) 派生 32 字节密钥，node.name /
//     node_content.content / node_blob.data 用 AES-256-GCM 逐值加密
//     （存储格式 nonce(12B) || ciphertext || tag；TEXT 列存 base64），
//     参数与校验值存 db_meta 表（加密 db 独有）。表结构与时间戳保持明文可读。
//
// 密码校验流程（db_open）：先无密码探测（读 sqlite_master），再按 db_meta 决定
// PasswordRequired / WrongPassword / 正常打开，错误以结构化 JSON 返回前端提示。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use rusqlite::Connection;
use sha2::Sha256;

use md5::{Digest, Md5};

// ====== 常量 ======

/// 文本文件扩展名：存 node_content；其余（图片等）存 node_blob。
const TEXT_EXTS: &[&str] = &["md", "markdown", "txt"];
/// PBKDF2 迭代次数（OWASP 对 PBKDF2-HMAC-SHA256 的推荐量级）。
const KDF_ITERS: u32 = 600_000;
/// 加密库密码校验明文标记（解密成功且等于该值 ⇒ 密码正确）。
const ENC_CHECK_PLAIN: &[u8] = b"vsnote-db-ok";
/// 导出/搜索等遍历的名称跳过规则（与 list_dir 的隐藏规则一致）。
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git"];

// ====== 全局注册表 ======

pub(crate) struct OpenedDb {
    conn: Connection,
    key: Option<[u8; 32]>,
}

fn registry() -> &'static Mutex<HashMap<String, OpenedDb>> {
    static DBS: OnceLock<Mutex<HashMap<String, OpenedDb>>> = OnceLock::new();
    DBS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 判定 path 是否属于某个已打开的 db 工作区，返回其根路径（即注册 key）。
/// 根本身（path == key）或根的子孙（key + '/' 或 '\' 前缀）都算命中；
/// 必须带分隔符前缀匹配，避免 `a.db` 误命中 `a.db2/...`。
pub fn match_root(path: &str) -> Option<String> {
    let map = registry().lock().ok()?;
    let mut best: Option<&String> = None;
    for key in map.keys() {
        let hit = path == key
            || path.starts_with(&format!("{}/", key))
            || path.starts_with(&format!("{}\\", key));
        if hit {
            let better = match &best {
                Some(b) => key.len() > b.len(),
                None => true,
            };
            if better {
                best = Some(key);
            }
        }
    }
    best.cloned()
}

// ====== 小工具 ======

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn is_text_ext(name: &str) -> bool {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    TEXT_EXTS.contains(&ext.as_str())
}

/// 节点名合法性：非空、不含路径分隔符、不是 . / ..
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

/// 结构化错误 JSON（与 lib.rs read_file 的错误格式一致，前端按 kind 映射错误码）。
fn io_err(kind: &str, path: &str, message: &str) -> String {
    format!(
        "{{\"kind\":\"{}\",\"path\":\"{}\",\"message\":\"{}\"}}",
        kind,
        path.replace('\\', "\\\\").replace('"', "\\\""),
        message.replace('"', "\\\"")
    )
}

/// 拆出 path 相对根的路径分量；path == 根时返回 None（表示根本身）。
fn rel_comps(root: &str, path: &str) -> Option<Vec<String>> {
    if path == root {
        return None;
    }
    let rel = path
        .strip_prefix(&format!("{}/", root))
        .or_else(|| path.strip_prefix(&format!("{}\\", root)))?;
    let comps: Vec<String> = rel
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if comps.iter().any(|c| !valid_name(c)) {
        return Some(Vec::new()); // 空向量表示非法路径，调用方按未找到处理
    }
    Some(comps)
}

// ====== 加密原语 ======

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, KDF_ITERS, &mut key);
    key
}

/// AES-256-GCM 加密，输出 nonce(12B) || ciphertext || tag。
fn seal(key: &[u8; 32], plain: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    use aes_gcm::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plain)
        .expect("AES-GCM encrypt infallible for valid key");
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    out
}

/// seal 的逆操作；密码错/数据损坏返回 None。
fn unseal(key: &[u8; 32], data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 13 {
        return None;
    }
    let cipher = Aes256Gcm::new(key.into());
    let plain = cipher
        .decrypt(Nonce::from_slice(&data[..12]), &data[12..])
        .ok()?;
    Some(plain)
}

/// name/content（TEXT 列）：加密时存 base64(nonce||ct)，明文时原样。
fn seal_text(key: &Option<[u8; 32]>, text: &str) -> String {
    match key {
        None => text.to_string(),
        Some(k) => base64::engine::general_purpose::STANDARD.encode(seal(k, text.as_bytes())),
    }
}

fn unseal_text(key: &Option<[u8; 32]>, text: &str) -> Option<String> {
    match key {
        None => Some(text.to_string()),
        Some(k) => {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(text)
                .ok()?;
            let bytes = unseal(k, &raw)?;
            String::from_utf8(bytes).ok()
        }
    }
}

/// node_blob.data（BLOB 列）：加密时存 nonce||ct 原始字节，明文时原样。
fn seal_blob(key: &Option<[u8; 32]>, data: &[u8]) -> Vec<u8> {
    match key {
        None => data.to_vec(),
        Some(k) => seal(k, data),
    }
}

fn unseal_blob(key: &Option<[u8; 32]>, data: &[u8]) -> Option<Vec<u8>> {
    match key {
        None => Some(data.to_vec()),
        Some(k) => unseal(k, data),
    }
}

// ====== db_meta 辅助（仅加密 db 使用） ======

const ENC_SALT_KEY: &str = "enc_salt";
const ENC_CHECK_KEY: &str = "enc_check";

fn get_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM db_meta WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO db_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

// ====== 打开 / 关闭 ======

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS node (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  parent_id INTEGER REFERENCES node(id) ON DELETE CASCADE,
  type TEXT NOT NULL CHECK (type IN ('folder', 'file')),
  name TEXT NOT NULL,
  create_time INTEGER NOT NULL,
  update_time INTEGER NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS node_content (
  node_id INTEGER PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
  content TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS node_blob (
  node_id INTEGER PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
  data BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_node_parent ON node(parent_id);
PRAGMA foreign_keys = ON;
";

fn open_conn(path: &str) -> Result<Connection, String> {
    Connection::open(path).map_err(|e| io_err("Io", path, &e.to_string()))
}

/// 打开（或创建）db 工作区并注册。返回 `{ encrypted, created }`。
/// 错误 kind：PasswordRequired（加密 db 未给密码）/ WrongPassword（密码错误）/
/// NotVsNoteDb（存在但不是本应用的 db）/ Io。
pub fn open(path: &str, password: Option<String>) -> Result<serde_json::Value, String> {
    let exists = Path::new(path).is_file();
    let empty = exists
        && std::fs::metadata(path)
            .map(|m| m.len() == 0)
            .unwrap_or(false);

    if !exists || empty {
        // 新建：给了密码 → 加密库；否则普通库
        let conn = open_conn(path)?;
        conn.execute_batch(SCHEMA_SQL).map_err(|e| io_err("Io", path, &e.to_string()))?;
        let mut key: Option<[u8; 32]> = None;
        if let Some(pw) = password.filter(|p| !p.is_empty()) {
            let mut salt = [0u8; 16];
            use aes_gcm::aead::rand_core::RngCore;
            OsRng.fill_bytes(&mut salt);
            let k = derive_key(&pw, &salt);
            set_meta(&conn, ENC_SALT_KEY, &base64::engine::general_purpose::STANDARD.encode(salt))
                .map_err(|e| io_err("Io", path, &e.to_string()))?;
            set_meta(
                &conn,
                ENC_CHECK_KEY,
                &base64::engine::general_purpose::STANDARD.encode(seal(&k, ENC_CHECK_PLAIN)),
            )
            .map_err(|e| io_err("Io", path, &e.to_string()))?;
            key = Some(k);
        }
        let encrypted = key.is_some();
        register(path, conn, key);
        return Ok(serde_json::json!({ "encrypted": encrypted, "created": true }));
    }

    // 已存在的库：探测是否本应用的 db / 是否加密
    let conn = open_conn(path)?;
    let node_table: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='node'",
            [],
            |r| r.get(0),
        )
        .ok();
    if node_table.is_none() {
        return Err(io_err("NotVsNoteDb", path, "不是有效的 vsnote SQLite db（缺少 node 表）"));
    }
    let salt_b64 = get_meta(&conn, ENC_SALT_KEY);
    match (salt_b64, password) {
        (None, _) => {
            // 明文库：忽略误输入的密码，按明文打开
            register(path, conn, None);
            Ok(serde_json::json!({ "encrypted": false, "created": false }))
        }
        (Some(_), None) => Err(io_err("PasswordRequired", path, "该 SQLite db 已加密，需要密码")),
        (Some(salt_b64), Some(pw)) => {
            if pw.is_empty() {
                return Err(io_err("PasswordRequired", path, "该 SQLite db 已加密，需要密码"));
            }
            let salt = base64::engine::general_purpose::STANDARD
                .decode(salt_b64.trim())
                .map_err(|_| io_err("Io", path, "db_meta.enc_salt 损坏"))?;
            let key = derive_key(&pw, &salt);
            let check_ok = get_meta(&conn, ENC_CHECK_KEY)
                .and_then(|c| base64::engine::general_purpose::STANDARD.decode(c.trim()).ok())
                .and_then(|c| unseal(&key, &c))
                .map(|p| p == ENC_CHECK_PLAIN)
                .unwrap_or(false);
            if !check_ok {
                return Err(io_err("WrongPassword", path, "密码错误"));
            }
            register(path, conn, Some(key));
            Ok(serde_json::json!({ "encrypted": true, "created": false }))
        }
    }
}

fn register(path: &str, conn: Connection, key: Option<[u8; 32]>) {
    if let Ok(mut map) = registry().lock() {
        // 重复打开同一 db：丢弃旧连接（drop 即关闭），换新连接
        map.insert(path.to_string(), OpenedDb { conn, key });
    }
}

pub fn close(path: &str) {
    if let Ok(mut map) = registry().lock() {
        map.remove(path);
    }
}

// ====== 节点解析 ======

/// 查找直接子节点（按解密后的名字比较）。返回 (id, type)。
fn find_child(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    parent: Option<i64>,
    name: &str,
) -> Option<(i64, String)> {
    let mut stmt = conn
        .prepare("SELECT id, name, type FROM node WHERE parent_id IS ?1 AND deleted = 0")
        .ok()?;
    let rows = stmt
        .query_map([parent], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })
        .ok()?;
    for row in rows {
        let (id, enc_name, ntype) = row.ok()?;
        if unseal_text(key, &enc_name).as_deref() == Some(name) {
            return Some((id, ntype));
        }
    }
    None
}

/// 按分量解析节点；comps 为空表示根（返回 (None, "folder")，id None 即根）。
/// 中间分量必须是文件夹。
fn resolve(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    comps: &[String],
) -> Option<(Option<i64>, String)> {
    let mut parent: Option<i64> = None;
    let mut ntype = "folder".to_string();
    for (i, c) in comps.iter().enumerate() {
        let (id, t) = find_child(conn, key, parent, c)?;
        if i + 1 < comps.len() && t != "folder" {
            return None;
        }
        parent = Some(id);
        ntype = t;
    }
    Some((parent, ntype))
}

/// 逐级创建文件夹链，返回最末一级的 parent id（None = 根级别）。
fn ensure_chain(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    comps: &[String],
    t: i64,
) -> Result<Option<i64>, String> {
    let mut parent: Option<i64> = None;
    for c in comps {
        if let Some((id, ntype)) = find_child(conn, key, parent, c) {
            if ntype != "folder" {
                return Err(io_err("Conflict", c, "同名文件已存在，无法创建文件夹"));
            }
            parent = Some(id);
        } else {
            conn.execute(
                "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
                 VALUES (?1, 'folder', ?2, ?3, ?3, 0)",
                rusqlite::params![parent, seal_text(key, c), t],
            )
            .map_err(|e| io_err("Io", c, &e.to_string()))?;
            parent = Some(conn.last_insert_rowid());
        }
    }
    Ok(parent)
}

/// 检查同一父节点下是否已有同名兄弟（排除自身 exclude）。
fn sibling_exists(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    parent: Option<i64>,
    name: &str,
    exclude: Option<i64>,
) -> bool {
    let mut stmt = match conn
        .prepare("SELECT id, name FROM node WHERE parent_id IS ?1 AND deleted = 0")
    {
        Ok(s) => s,
        Err(_) => return false,
    };
    let rows = match stmt.query_map([parent], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return false,
    };
    for row in rows {
        let (id, enc_name) = match row {
            Ok(v) => v,
            Err(_) => continue,
        };
        if Some(id) == exclude {
            continue;
        }
        if unseal_text(key, &enc_name).as_deref() == Some(name) {
            return true;
        }
    }
    false
}

/// 触碰节点及各祖先的 update_time（保存/重命名/移动后让 file_meta 变化）。
fn touch_ancestors(conn: &Connection, mut id: Option<i64>, t: i64) {
    while let Some(cur) = id {
        let parent: Option<Option<i64>> = conn
            .query_row(
                "SELECT parent_id FROM node WHERE id = ?1",
                [cur],
                |r| r.get(0),
            )
            .ok();
        let _ = conn.execute(
            "UPDATE node SET update_time = ?2 WHERE id = ?1",
            rusqlite::params![cur, t],
        );
        match parent {
            Some(p) => id = p,
            None => break,
        }
    }
}

// ====== 路由实现（与 lib.rs 命令一一对应） ======

pub fn read_file(root: &str, path: &str) -> Result<String, String> {
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = match rel_comps(root, path) {
        None => return Err(io_err("NotFound", path, "workspace root has no content")),
        Some(c) if c.is_empty() => return Err(io_err("NotFound", path, "invalid path")),
        Some(c) => c,
    };
    let (id, ntype) = resolve(&db.conn, &db.key, &comps)
        .ok_or_else(|| io_err("NotFound", path, "node not found"))?;
    if ntype != "file" {
        return Err(io_err("NotFound", path, "path is a folder, not a file"));
    }
    let _ = id;
    let content: Option<String> = db
        .conn
        .query_row(
            "SELECT content FROM node_content WHERE node_id = ?1",
            [id],
            |r| r.get(0),
        )
        .ok();
    match content {
        Some(enc) => unseal_text(&db.key, &enc)
            .ok_or_else(|| io_err("Io", path, "decrypt failed (wrong password?)")),
        None => Err(io_err("NotFound", path, "file has no text content")),
    }
}

/// 读取文件的原始字节（图片等二进制）。仅用于 fetch_image_as_base64 路由。
pub fn read_file_bytes(root: &str, path: &str) -> Result<Vec<u8>, String> {
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = rel_comps(root, path)
        .ok_or_else(|| io_err("NotFound", path, "workspace root is not an image"))?;
    if comps.is_empty() {
        return Err(io_err("NotFound", path, "invalid path"));
    }
    let (id, ntype) = resolve(&db.conn, &db.key, &comps)
        .ok_or_else(|| io_err("NotFound", path, "node not found"))?;
    if ntype != "file" {
        return Err(io_err("NotFound", path, "path is a folder, not a file"));
    }
    // 优先 blob（二进制）；没有 blob 时尝试文本内容
    let blob: Option<Vec<u8>> = db
        .conn
        .query_row(
            "SELECT data FROM node_blob WHERE node_id = ?1",
            [id],
            |r| r.get(0),
        )
        .ok();
    if let Some(enc) = blob {
        return unseal_blob(&db.key, &enc)
            .ok_or_else(|| io_err("Io", path, "decrypt failed (wrong password?)"));
    }
    let text: Option<String> = db
        .conn
        .query_row(
            "SELECT content FROM node_content WHERE node_id = ?1",
            [id],
            |r| r.get(0),
        )
        .ok();
    match text {
        Some(enc) => unseal_text(&db.key, &enc)
            .map(|s| s.into_bytes())
            .ok_or_else(|| io_err("Io", path, "decrypt failed (wrong password?)")),
        None => Err(io_err("NotFound", path, "file has no content")),
    }
}

pub fn write_file(root: &str, path: &str, content: &str) -> Result<(), String> {
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = rel_comps(root, path).ok_or_else(|| io_err("Io", path, "cannot write workspace root"))?;
    let (name, parents) = comps.split_last().ok_or_else(|| io_err("Io", path, "invalid path"))?;
    if !valid_name(name) {
        return Err(io_err("Io", path, "invalid file name"));
    }
    let t = now_ms();
    let parent = ensure_chain(&db.conn, &db.key, parents, t)?;
    match resolve(&db.conn, &db.key, &comps) {
        Some((id, _)) => {
            // 已存在：覆盖文本内容
            let id = id.ok_or_else(|| io_err("Io", path, "cannot write workspace root"))?;
            db.conn
                .execute(
                    "INSERT INTO node_content (node_id, content) VALUES (?1, ?2)
                     ON CONFLICT(node_id) DO UPDATE SET content = excluded.content",
                    rusqlite::params![id, seal_text(&db.key, content)],
                )
                .map_err(|e| io_err("Io", path, &e.to_string()))?;
            let _ = db.conn.execute(
                "UPDATE node SET update_time = ?2 WHERE id = ?1",
                rusqlite::params![id, t],
            );
            let _ = db.conn.execute(
                "DELETE FROM node_blob WHERE node_id = ?1",
                [id],
            );
            touch_ancestors(&db.conn, Some(id), t);
            Ok(())
        }
        None => {
            db.conn
                .execute(
                    "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
                     VALUES (?1, 'file', ?2, ?3, ?3, 0)",
                    rusqlite::params![parent, seal_text(&db.key, name), t],
                )
                .map_err(|e| io_err("Io", path, &e.to_string()))?;
            let id = db.conn.last_insert_rowid();
            db.conn
                .execute(
                    "INSERT INTO node_content (node_id, content) VALUES (?1, ?2)",
                    rusqlite::params![id, seal_text(&db.key, content)],
                )
                .map_err(|e| io_err("Io", path, &e.to_string()))?;
            touch_ancestors(&db.conn, Some(id), t);
            Ok(())
        }
    }
}

pub fn list_dir(root: &str, path: &str) -> Result<Vec<crate::DirEntryInfo>, String> {
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = rel_comps(root, path).unwrap_or_default();
    let (parent, ntype) = if comps.is_empty() {
        (None, "folder".to_string())
    } else {
        resolve(&db.conn, &db.key, &comps)
            .ok_or_else(|| io_err("NotFound", path, "node not found"))?
    };
    if ntype != "folder" {
        return Ok(Vec::new());
    }
    let mut stmt = db
        .conn
        .prepare("SELECT id, name, type FROM node WHERE parent_id IS ?1 AND deleted = 0")
        .map_err(|e| io_err("Io", path, &e.to_string()))?;
    let rows = stmt
        .query_map([parent], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })
        .map_err(|e| io_err("Io", path, &e.to_string()))?;
    let mut entries: Vec<(String, bool)> = Vec::new(); // (name, is_dir)
    for row in rows {
        let (_id, enc_name, ntype) = row.map_err(|e| io_err("Io", path, &e.to_string()))?;
        let name = match unseal_text(&db.key, &enc_name) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        let is_dir = ntype == "folder";
        if !is_dir && !is_text_ext(&name) {
            continue;
        }
        entries.push((name, is_dir));
    }
    // 文件夹在前，名称不区分大小写升序（与 list_dir 的文件系统排序一致）
    entries.sort_by(|a, b| {
        if a.1 != b.1 {
            return if a.1 { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        a.0.to_lowercase().cmp(&b.0.to_lowercase())
    });
    Ok(entries
        .into_iter()
        .map(|(name, is_dir)| {
            let path = format!("{}/{}", root, name);
            crate::DirEntryInfo { name, path, is_dir }
        })
        .collect())
}

pub fn is_directory(root: &str, path: &str) -> bool {
    let map = match registry().lock() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let db = match map.get(root) {
        Some(d) => d,
        None => return false,
    };
    match rel_comps(root, path) {
        None => true, // 根本身是「文件夹」
        Some(comps) if comps.is_empty() => false,
        Some(comps) => matches!(resolve(&db.conn, &db.key, &comps), Some((_, t)) if t == "folder"),
    }
}

pub fn file_meta(root: &str, path: &str) -> Result<Option<crate::FileMeta>, String> {
    if path == root {
        // 根：返回 db 文件本身的真实元数据
        return Ok(std::fs::metadata(path).ok().and_then(|m| {
            m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| {
                crate::FileMeta { mtime: d.as_millis() as u64, size: m.len() }
            })
        }));
    }
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = match rel_comps(root, path) {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(None),
    };
    let (id, ntype) = match resolve(&db.conn, &db.key, &comps) {
        Some(v) => v,
        None => return Ok(None),
    };
    let id = match id {
        Some(i) => i,
        None => return Ok(None),
    };
    let row: Option<(i64, Option<String>, Option<Vec<u8>>)> = db
        .conn
        .query_row(
            "SELECT n.update_time, c.content, b.data
             FROM node n
             LEFT JOIN node_content c ON c.node_id = n.id
             LEFT JOIN node_blob b ON b.node_id = n.id
             WHERE n.id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )
        .ok();
    let Some((update_time, content, blob)) = row else {
        return Ok(None);
    };
    let size = if ntype == "folder" {
        0
    } else if let Some(enc) = blob {
        unseal_blob(&db.key, &enc).map(|b| b.len()).unwrap_or(0) as u64
    } else if let Some(enc) = content {
        unseal_text(&db.key, &enc).map(|s| s.len()).unwrap_or(0) as u64
    } else {
        0
    };
    Ok(Some(crate::FileMeta { mtime: update_time as u64, size }))
}

pub fn ensure_dir(root: &str, path: &str) -> Result<(), String> {
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = rel_comps(root, path).ok_or_else(|| io_err("Io", path, "workspace root exists"))?;
    if comps.is_empty() {
        return Ok(());
    }
    for c in &comps {
        if !valid_name(c) {
            return Err(io_err("Io", path, "invalid folder name"));
        }
    }
    let t = now_ms();
    let parent = ensure_chain(&db.conn, &db.key, &comps, t)?;
    if let Some(id) = parent {
        touch_ancestors(&db.conn, Some(id), t);
    }
    Ok(())
}

pub fn rename_path(root: &str, from: &str, to: &str) -> Result<(), String> {
    let map = registry().lock().map_err(|_| io_err("Io", from, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", from, "db workspace not open"))?;
    let from_comps = rel_comps(root, from).ok_or_else(|| io_err("Io", from, "cannot rename workspace root"))?;
    let (_fname, fparents) = from_comps
        .split_last()
        .ok_or_else(|| io_err("Io", from, "invalid path"))?;
    let to_comps = rel_comps(root, to).ok_or_else(|| io_err("Io", to, "cannot rename to workspace root"))?;
    let (tname, tparents) = to_comps
        .split_last()
        .ok_or_else(|| io_err("Io", to, "invalid path"))?;
    if !valid_name(tname) {
        return Err(io_err("Io", to, "invalid name"));
    }
    let t = now_ms();
    // 仅支持同目录重命名（与前端行为一致）
    if fparents != tparents {
        return Err(io_err("Io", to, "rename across folders is not supported"));
    }
    let (id, _) = resolve(&db.conn, &db.key, &from_comps)
        .ok_or_else(|| io_err("NotFound", from, "node not found"))?;
    let id = id.ok_or_else(|| io_err("Io", from, "cannot rename workspace root"))?;
    let parent_id = resolve_parent(&db.conn, &db.key, fparents)?;
    if sibling_exists(&db.conn, &db.key, parent_id, tname, Some(id)) {
        return Err(io_err("Conflict", to, "target name already exists"));
    }
    db.conn
        .execute(
            "UPDATE node SET name = ?2, update_time = ?3 WHERE id = ?1",
            rusqlite::params![id, seal_text(&db.key, tname), t],
        )
        .map_err(|e| io_err("Io", from, &e.to_string()))?;
    touch_ancestors(&db.conn, Some(id), t);
    Ok(())
}

/// 解析父链 id（假设链已存在；仅 rename 用）。
fn resolve_parent(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    comps: &[String],
) -> Result<Option<i64>, String> {
    let mut parent: Option<i64> = None;
    for c in comps {
        match find_child(conn, key, parent, c) {
            Some((id, _)) => parent = Some(id),
            None => return Err(io_err("NotFound", c, "parent folder not found")),
        }
    }
    Ok(parent)
}

pub fn remove_path(root: &str, path: &str) -> Result<(), String> {
    let map = registry().lock().map_err(|_| io_err("Io", path, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", path, "db workspace not open"))?;
    let comps = rel_comps(root, path).ok_or_else(|| io_err("Io", path, "cannot delete workspace root"))?;
    if comps.is_empty() {
        return Err(io_err("Io", path, "cannot delete workspace root"));
    }
    let (id, _) = resolve(&db.conn, &db.key, &comps)
        .ok_or_else(|| io_err("NotFound", path, "node not found"))?;
    let id = id.ok_or_else(|| io_err("Io", path, "cannot delete workspace root"))?;
    // FK ON DELETE CASCADE 级联删除子孙与 node_content / node_blob
    db.conn
        .execute("DELETE FROM node WHERE id = ?1", [id])
        .map_err(|e| io_err("Io", path, &e.to_string()))?;
    let t = now_ms();
    let parent = comps[..comps.len() - 1].to_vec();
    let pid = resolve(&db.conn, &db.key, &parent).and_then(|(i, _)| i);
    touch_ancestors(&db.conn, pid, t);
    Ok(())
}

/// 深拷贝节点子树（src_parent 下的所有孩子 → dst_parent）。
fn copy_children(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    src_parent: Option<i64>,
    dst_parent: Option<i64>,
    t: i64,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id, name, type FROM node WHERE parent_id IS ?1 AND deleted = 0")
        .map_err(|e| io_err("Io", "", &e.to_string()))?;
    let rows = stmt
        .query_map([src_parent], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })
        .map_err(|e| io_err("Io", "", &e.to_string()))?;
    let mut copied: Vec<(String, String, i64)> = Vec::new();
    for row in rows {
        let (id, enc_name, ntype) = row.map_err(|e| io_err("Io", "", &e.to_string()))?;
        copied.push((enc_name, ntype, id));
    }
    drop(stmt);
    for (enc_name, ntype, id) in copied {
        conn.execute(
            "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
             VALUES (?1, ?2, ?3, ?4, ?4, 0)",
            rusqlite::params![dst_parent, ntype, enc_name, t],
        )
        .map_err(|e| io_err("Io", "", &e.to_string()))?;
        let new_id = conn.last_insert_rowid();
        if ntype == "folder" {
            copy_children(conn, key, Some(id), Some(new_id), t)?;
        } else {
            conn.execute(
                "INSERT INTO node_content (node_id, content)
                 SELECT ?1, content FROM node_content WHERE node_id = ?2",
                rusqlite::params![new_id, id],
            )
            .map_err(|e| io_err("Io", "", &e.to_string()))?;
            conn.execute(
                "INSERT INTO node_blob (node_id, data)
                 SELECT ?1, data FROM node_blob WHERE node_id = ?2",
                rusqlite::params![new_id, id],
            )
            .map_err(|e| io_err("Io", "", &e.to_string()))?;
        }
    }
    Ok(())
}

pub fn copy_path(root: &str, from: &str, to: &str) -> Result<(), String> {
    let map = registry().lock().map_err(|_| io_err("Io", from, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", from, "db workspace not open"))?;
    let from_comps = rel_comps(root, from).ok_or_else(|| io_err("Io", from, "cannot copy workspace root"))?;
    let to_comps = rel_comps(root, to).ok_or_else(|| io_err("Io", to, "invalid target"))?;
    if to_comps.is_empty() {
        return Err(io_err("Io", to, "cannot copy onto workspace root"));
    }
    let (tname, tparents) = to_comps
        .split_last()
        .ok_or_else(|| io_err("Io", to, "invalid target"))?;
    if !valid_name(tname) {
        return Err(io_err("Io", to, "invalid name"));
    }
    let t = now_ms();
    let (src_id, src_type) = resolve(&db.conn, &db.key, &from_comps)
        .ok_or_else(|| io_err("NotFound", from, "node not found"))?;
    if sibling_exists(&db.conn, &db.key, ensure_chain(&db.conn, &db.key, tparents, t)?, tname, None) {
        return Err(io_err("Conflict", to, "target name already exists"));
    }
    let dst_parent = ensure_chain(&db.conn, &db.key, tparents, t)?;
    // 在目标处创建副本根节点
    db.conn
        .execute(
            "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
             VALUES (?1, ?2, ?3, ?4, ?4, 0)",
            rusqlite::params![dst_parent, src_type, seal_text(&db.key, tname), t],
        )
        .map_err(|e| io_err("Io", to, &e.to_string()))?;
    let new_id = db.conn.last_insert_rowid();
    if src_type == "folder" {
        let src = src_id.ok_or_else(|| io_err("Io", from, "cannot copy workspace root"))?;
        copy_children(&db.conn, &db.key, Some(src), Some(new_id), t)?;
    } else {
        db.conn
            .execute(
                "INSERT INTO node_content (node_id, content)
                 SELECT ?1, content FROM node_content WHERE node_id = ?2",
                rusqlite::params![new_id, src_id],
            )
            .map_err(|e| io_err("Io", to, &e.to_string()))?;
        db.conn
            .execute(
                "INSERT INTO node_blob (node_id, data)
                 SELECT ?1, data FROM node_blob WHERE node_id = ?2",
                rusqlite::params![new_id, src_id],
            )
            .map_err(|e| io_err("Io", to, &e.to_string()))?;
    }
    touch_ancestors(&db.conn, Some(new_id), t);
    Ok(())
}

pub fn move_path(root: &str, from: &str, to: &str) -> Result<(), String> {
    let map = registry().lock().map_err(|_| io_err("Io", from, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", from, "db workspace not open"))?;
    let from_comps = rel_comps(root, from).ok_or_else(|| io_err("Io", from, "cannot move workspace root"))?;
    let to_comps = rel_comps(root, to).ok_or_else(|| io_err("Io", to, "invalid target"))?;
    if from_comps.is_empty() || to_comps.is_empty() {
        return Err(io_err("Io", to, "cannot move workspace root"));
    }
    let (tname, tparents) = to_comps
        .split_last()
        .ok_or_else(|| io_err("Io", to, "invalid target"))?;
    if !valid_name(tname) {
        return Err(io_err("Io", to, "invalid name"));
    }
    let t = now_ms();
    let (src_id, _) = resolve(&db.conn, &db.key, &from_comps)
        .ok_or_else(|| io_err("NotFound", from, "node not found"))?;
    let src_id = src_id.ok_or_else(|| io_err("Io", from, "cannot move workspace root"))?;
    // 环检查：目标父链不能包含源节点自身
    let dst_parent = ensure_chain(&db.conn, &db.key, tparents, t)?;
    if let Some(mut cur) = dst_parent {
        while cur != 0 {
            if cur == src_id {
                return Err(io_err("Io", to, "cannot move a folder into itself"));
            }
            let p: Option<Option<i64>> = db
                .conn
                .query_row("SELECT parent_id FROM node WHERE id = ?1", [cur], |r| r.get(0))
                .ok();
            match p {
                Some(Some(v)) => cur = v,
                _ => break,
            }
        }
    }
    if sibling_exists(&db.conn, &db.key, dst_parent, tname, Some(src_id)) {
        return Err(io_err("Conflict", to, "target name already exists"));
    }
    db.conn
        .execute(
            "UPDATE node SET parent_id = ?2, name = ?3, update_time = ?4 WHERE id = ?1",
            rusqlite::params![src_id, dst_parent, seal_text(&db.key, tname), t],
        )
        .map_err(|e| io_err("Io", from, &e.to_string()))?;
    touch_ancestors(&db.conn, Some(src_id), t);
    Ok(())
}

// ====== 全局搜索（search_in_files 路由） ======

pub fn search_in_files(
    root: &str,
    dir: &str,
    pattern: &str,
    case_sensitive: bool,
    use_regex: bool,
    extensions: &[String],
) -> Result<Vec<crate::FileMatch>, String> {
    if pattern.is_empty() {
        return Ok(Vec::new());
    }
    let re: Option<regex::Regex> = if use_regex {
        match regex::Regex::new(pattern) {
            Ok(r) => Some(r),
            Err(_) => {
                return Err("{\"kind\":\"InvalidEncoding\",\"path\":\"\",\"message\":\"invalid regex\"}".into())
            }
        }
    } else {
        None
    };
    let pattern_lower = pattern.to_lowercase();
    let exts: Vec<String> = if extensions.is_empty() {
        TEXT_EXTS.iter().map(|s| s.to_string()).collect()
    } else {
        extensions.iter().map(|e| e.to_lowercase()).collect()
    };

    const MAX_FILES: usize = 2000;
    const MAX_PER_FILE: usize = 500;
    const MAX_TOTAL: usize = 5000;
    const MAX_LINE_TEXT: usize = 300;

    let map = registry().lock().map_err(|e| e.to_string())?;
    let db = map.get(root).ok_or_else(|| "db workspace not open".to_string())?;

    // 定位搜索子树根，收集 (node_id, 完整虚拟路径)
    let dir_comps = rel_comps(root, dir).unwrap_or_default();
    let mut files: Vec<(i64, String)> = Vec::new();
    if dir_comps.is_empty() {
        collect_file_nodes(&db.conn, &db.key, root, dir, None, &exts, &mut files);
    } else {
        match resolve(&db.conn, &db.key, &dir_comps) {
            Some((Some(id), t)) if t == "folder" => {
                collect_file_nodes(&db.conn, &db.key, root, dir, Some(id), &exts, &mut files);
            }
            Some((Some(id), _)) => {
                // 直接指定了某个文件节点：按扩展名过滤后单搜它
                let name = dir_comps.last().cloned().unwrap_or_default();
                let ext = Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if exts.iter().any(|e| *e == ext) {
                    files.push((id, format!("{}/{}", root, name)));
                }
            }
            _ => return Ok(Vec::new()),
        }
    }

    let mut results: Vec<crate::FileMatch> = Vec::new();
    let mut total = 0usize;
    for (id, full_path) in files {
        if total >= MAX_TOTAL || results.len() >= MAX_FILES {
            break;
        }
        let content: Option<String> = db
            .conn
            .query_row(
                "SELECT content FROM node_content WHERE node_id = ?1",
                [id],
                |r| r.get(0),
            )
            .ok();
        let Some(enc) = content else { continue };
        let Some(text) = unseal_text(&db.key, &enc) else { continue };
        let mut file_matches: Vec<crate::LineMatch> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            if file_matches.len() >= MAX_PER_FILE {
                break;
            }
            let byte_col: Option<usize> = if let Some(re) = &re {
                re.find(line).map(|m| m.start())
            } else if case_sensitive {
                line.find(&pattern[..])
            } else {
                line.to_lowercase().find(&pattern_lower)
            };
            if let Some(bc) = byte_col {
                let char_col = line[..bc].chars().count();
                let line_text: String = line.chars().take(MAX_LINE_TEXT).collect();
                file_matches.push(crate::LineMatch {
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
            results.push(crate::FileMatch { path: full_path, matches: file_matches });
        }
    }
    Ok(results)
}

/// 递归收集 parent 子树下所有 file 节点（id + 完整虚拟路径），按扩展名过滤。
/// dir_path 是 parent 对应的虚拟目录路径（root 或其子目录）。
fn collect_file_nodes(
    conn: &Connection,
    key: &Option<[u8; 32]>,
    root: &str,
    dir_path: &str,
    parent: Option<i64>,
    exts: &[String],
    out: &mut Vec<(i64, String)>,
) {
    let mut stmt = match conn
        .prepare("SELECT id, name, type FROM node WHERE parent_id IS ?1 AND deleted = 0")
    {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows = match stmt.query_map([parent], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    }) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut children: Vec<(i64, String, String)> = Vec::new();
    for row in rows {
        if let Ok((id, enc_name, ntype)) = row {
            children.push((id, enc_name, ntype));
        }
    }
    drop(stmt);
    let _ = root;
    for (id, enc_name, ntype) in children {
        let name = match unseal_text(key, &enc_name) {
            Some(n) => n,
            None => continue,
        };
        if ntype == "folder" {
            let child_dir = format!("{}/{}", dir_path, name);
            collect_file_nodes(conn, key, root, &child_dir, Some(id), exts, out);
        } else {
            let ext = Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if exts.iter().any(|e| *e == ext) {
                out.push((id, format!("{}/{}", dir_path, name)));
            }
        }
    }
}

// ====== 图片资源（save_image_to_assets / fetch_image_as_base64 路由） ======

pub fn save_image_to_assets(
    root: &str,
    assets_dir: &str,
    bytes: &[u8],
    ext: &str,
) -> Result<crate::ImageAssetInfo, String> {
    let map = registry().lock().map_err(|_| io_err("Io", assets_dir, "db registry poisoned"))?;
    let db = map.get(root).ok_or_else(|| io_err("NotFound", assets_dir, "db workspace not open"))?;
    let comps = rel_comps(root, assets_dir)
        .ok_or_else(|| io_err("Io", assets_dir, "invalid assets dir"))?;
    let t = now_ms();
    let parent = ensure_chain(&db.conn, &db.key, &comps, t)?;
    let hash = format!("{:x}", Md5::digest(bytes));
    let filename = format!("{}.{}", hash, ext);
    if find_child(&db.conn, &db.key, parent, &filename).is_none() {
        db.conn
            .execute(
                "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
                 VALUES (?1, 'file', ?2, ?3, ?3, 0)",
                rusqlite::params![parent, seal_text(&db.key, &filename), t],
            )
            .map_err(|e| io_err("Io", assets_dir, &e.to_string()))?;
        let id = db.conn.last_insert_rowid();
        db.conn
            .execute(
                "INSERT INTO node_blob (node_id, data) VALUES (?1, ?2)",
                rusqlite::params![id, seal_blob(&db.key, bytes)],
            )
            .map_err(|e| io_err("Io", assets_dir, &e.to_string()))?;
        touch_ancestors(&db.conn, Some(id), t);
    }
    let (width, height) = match imagesize::blob_size(bytes) {
        Ok(size) => (size.width as u32, size.height as u32),
        Err(_) => (0, 0),
    };
    Ok(crate::ImageAssetInfo { filename, width, height })
}

// ====== 导出：文件夹 / SQLite db 互转 ======

/// 复制源节点树到目标库（支持明文/加密之间的任意转换：读取时用 src_key 解，
/// 写入时用 dst_key 加）。
fn copy_tree_into(
    src: &Connection,
    src_key: &Option<[u8; 32]>,
    src_parent: Option<i64>,
    dst: &Connection,
    dst_key: &Option<[u8; 32]>,
    dst_parent: Option<i64>,
) -> Result<usize, String> {
    let mut stmt = src
        .prepare("SELECT id, type, name, create_time, update_time FROM node WHERE parent_id IS ?1 AND deleted = 0")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([src_parent], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut nodes: Vec<(i64, String, String, i64, i64)> = Vec::new();
    for row in rows {
        nodes.push(row.map_err(|e| e.to_string())?);
    }
    drop(stmt);

    let mut count = 0usize;
    for (id, ntype, enc_name, ctime, utime) in nodes {
        let name = unseal_text(src_key, &enc_name).ok_or_else(|| "节点名解密失败".to_string())?;
        dst.execute(
            "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            rusqlite::params![dst_parent, ntype, seal_text(dst_key, &name), ctime, utime],
        )
        .map_err(|e| e.to_string())?;
        let new_id = dst.last_insert_rowid();
        count += 1;
        if ntype == "folder" {
            count += copy_tree_into(src, src_key, Some(id), dst, dst_key, Some(new_id))?;
        } else {
            // 文本内容
            if let Ok(content) = src.query_row(
                "SELECT content FROM node_content WHERE node_id = ?1",
                [id],
                |r| r.get::<_, String>(0),
            ) {
                if let Some(text) = unseal_text(src_key, &content) {
                    dst.execute(
                        "INSERT INTO node_content (node_id, content) VALUES (?1, ?2)",
                        rusqlite::params![new_id, seal_text(dst_key, &text)],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            // 二进制内容
            if let Ok(data) = src.query_row(
                "SELECT data FROM node_blob WHERE node_id = ?1",
                [id],
                |r| r.get::<_, Vec<u8>>(0),
            ) {
                if let Some(raw) = unseal_blob(src_key, &data) {
                    dst.execute(
                        "INSERT INTO node_blob (node_id, data) VALUES (?1, ?2)",
                        rusqlite::params![new_id, seal_blob(dst_key, &raw)],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(count)
}

/// 把文件系统目录树写入目标库。
fn copy_fs_into(
    dir: &Path,
    dst: &Connection,
    dst_key: &Option<[u8; 32]>,
    dst_parent: Option<i64>,
) -> Result<usize, String> {
    let rd = fs_read_dir(dir)?;
    let mut count = 0usize;
    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let ctime = filetime_ms(&meta, true);
        let utime = filetime_ms(&meta, false);
        if meta.is_dir() {
            dst.execute(
                "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
                 VALUES (?1, 'folder', ?2, ?3, ?4, 0)",
                rusqlite::params![dst_parent, seal_text(dst_key, &name), ctime, utime],
            )
            .map_err(|e| e.to_string())?;
            let id = dst.last_insert_rowid();
            count += 1;
            count += copy_fs_into(&path, dst, dst_key, Some(id))?;
        } else {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            dst.execute(
                "INSERT INTO node (parent_id, type, name, create_time, update_time, deleted)
                 VALUES (?1, 'file', ?2, ?3, ?4, 0)",
                rusqlite::params![dst_parent, seal_text(dst_key, &name), ctime, utime],
            )
            .map_err(|e| e.to_string())?;
            let id = dst.last_insert_rowid();
            if is_text_ext(&name) {
                let text = crate::decode_bytes(&bytes);
                dst.execute(
                    "INSERT INTO node_content (node_id, content) VALUES (?1, ?2)",
                    rusqlite::params![id, seal_text(dst_key, &text)],
                )
                .map_err(|e| e.to_string())?;
            } else {
                dst.execute(
                    "INSERT INTO node_blob (node_id, data) VALUES (?1, ?2)",
                    rusqlite::params![id, seal_blob(dst_key, &bytes)],
                )
                .map_err(|e| e.to_string())?;
            }
            count += 1;
        }
    }
    Ok(count)
}

fn fs_read_dir(dir: &Path) -> Result<std::fs::ReadDir, String> {
    std::fs::read_dir(dir).map_err(|e| e.to_string())
}

fn filetime_ms(meta: &std::fs::Metadata, created: bool) -> i64 {
    let t = if created {
        meta.created().or_else(|_| meta.modified()).ok()
    } else {
        meta.modified().ok()
    };
    t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 创建目标库（可选加密），建表。目标存在则报错。
fn create_dest_db(dest: &str, password: &Option<String>) -> Result<(Connection, Option<[u8; 32]>), String> {
    if Path::new(dest).exists() {
        return Err(format!("目标文件已存在: {}", dest));
    }
    let conn = open_conn(dest)?;
    conn.execute_batch(SCHEMA_SQL).map_err(|e| e.to_string())?;
    match password {
        Some(pw) if !pw.is_empty() => {
            let mut salt = [0u8; 16];
            use aes_gcm::aead::rand_core::RngCore;
            OsRng.fill_bytes(&mut salt);
            let key = derive_key(pw, &salt);
            set_meta(&conn, ENC_SALT_KEY, &base64::engine::general_purpose::STANDARD.encode(salt))
                .map_err(|e| e.to_string())?;
            set_meta(
                &conn,
                ENC_CHECK_KEY,
                &base64::engine::general_purpose::STANDARD.encode(seal(&key, ENC_CHECK_PLAIN)),
            )
            .map_err(|e| e.to_string())?;
            Ok((conn, Some(key)))
        }
        _ => Ok((conn, None)),
    }
}

/// 导出为 SQLite db。src 可以是文件夹或已打开的 db 工作区根。
pub fn export_to_db(src: &str, dest: &str, password: Option<String>) -> Result<serde_json::Value, String> {
    let _ = crate::safe_write_target(dest)?;
    if let Some(root) = match_root(src) {
        // 源是 db 工作区：从注册连接复制
        let map = registry().lock().map_err(|_| "db registry poisoned".to_string())?;
        let db = map.get(&root).ok_or_else(|| "db workspace not open".to_string())?;
        let (dst_conn, dst_key) = create_dest_db(dest, &password)?;
        let count = copy_tree_into(&db.conn, &db.key, None, &dst_conn, &dst_key, None)?;
        return Ok(serde_json::json!({ "count": count, "encrypted": dst_key.is_some() }));
    }
    // 源是文件夹
    if !Path::new(src).is_dir() {
        return Err(format!("源不是文件夹或已打开的 SQLite db: {}", src));
    }
    let (dst_conn, dst_key) = create_dest_db(dest, &password)?;
    let count = copy_fs_into(Path::new(src), &dst_conn, &dst_key, None)?;
    Ok(serde_json::json!({ "count": count, "encrypted": dst_key.is_some() }))
}

/// 把（已打开的）db 工作区导出为文件夹。
pub fn export_to_folder(src: &str, dest: &str) -> Result<serde_json::Value, String> {
    let root = match_root(src).ok_or_else(|| format!("源不是已打开的 SQLite db 工作区: {}", src))?;
    let map = registry().lock().map_err(|_| "db registry poisoned".to_string())?;
    let db = map.get(&root).ok_or_else(|| "db workspace not open".to_string())?;

    let dest_path = Path::new(dest);
    if dest_path.exists() {
        let empty = std::fs::read_dir(dest_path)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if !empty {
            return Err(format!("目标文件夹非空: {}", dest));
        }
    } else {
        std::fs::create_dir_all(dest_path).map_err(|e| e.to_string())?;
    }

    // 递归导出（闭包递归不好写，用显式栈）
    let mut count = 0usize;
    let mut stack: Vec<(Option<i64>, std::path::PathBuf)> = vec![(None, dest_path.to_path_buf())];
    while let Some((parent, dir_path)) = stack.pop() {
        let mut stmt = db
            .conn
            .prepare("SELECT id, type, name, create_time, update_time FROM node WHERE parent_id IS ?1 AND deleted = 0")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([parent], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut children: Vec<(i64, String, String, i64, i64)> = Vec::new();
        for row in rows {
            children.push(row.map_err(|e| e.to_string())?);
        }
        drop(stmt);
        for (id, ntype, enc_name, ctime, utime) in children {
            let name = unseal_text(&db.key, &enc_name).ok_or_else(|| "节点名解密失败".to_string())?;
            let child_path = dir_path.join(&name);
            if ntype == "folder" {
                std::fs::create_dir_all(&child_path).map_err(|e| e.to_string())?;
                count += 1;
                stack.push((Some(id), child_path));
            } else {
                // 文本
                let text: Option<String> = db
                    .conn
                    .query_row(
                        "SELECT content FROM node_content WHERE node_id = ?1",
                        [id],
                        |r| r.get(0),
                    )
                    .ok();
                if let Some(enc) = text {
                    let plain = unseal_text(&db.key, &enc).ok_or_else(|| "内容解密失败".to_string())?;
                    std::fs::write(&child_path, plain).map_err(|e| e.to_string())?;
                } else {
                    let data: Option<Vec<u8>> = db
                        .conn
                        .query_row(
                            "SELECT data FROM node_blob WHERE node_id = ?1",
                            [id],
                            |r| r.get(0),
                        )
                        .ok();
                    if let Some(enc) = data {
                        let raw = unseal_blob(&db.key, &enc).ok_or_else(|| "内容解密失败".to_string())?;
                        std::fs::write(&child_path, raw).map_err(|e| e.to_string())?;
                    }
                }
                count += 1;
            }
            let _ = (ctime, utime);
        }
    }
    Ok(serde_json::json!({ "count": count }))
}
