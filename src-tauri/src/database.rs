use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

/// SQLite 数据库状态
pub struct DatabaseState {
    pub conn: Mutex<Connection>,
}

/// 搜索历史记录
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchHistory {
    pub id: i64,
    pub word: String,
    pub searched_at: String,
}

/// 收藏记录
#[derive(Debug, Serialize, Deserialize)]
pub struct Favorite {
    pub id: i64,
    pub word: String,
    pub created_at: String,
}

/// 单词学习记录
#[derive(Debug, Serialize, Deserialize)]
pub struct LearningRecord {
    pub id: i64,
    pub word: String,
    pub familiarity: i64,
    pub next_review_at: String,
    pub updated_at: String,
}

/// 单词复习历史
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewHistory {
    pub id: i64,
    pub word: String,
    pub familiarity: i64,
    pub reviewed_at: String,
}

/// 应用数据备份结构
#[derive(Debug, Serialize, Deserialize)]
struct AppBackup {
    format: String,
    schema_version: u32,
    exported_at: String,

    search_history: Vec<SearchHistory>,
    favorites: Vec<Favorite>,
    learning_records: Vec<LearningRecord>,
    review_history: Vec<ReviewHistory>,
}

/// 备份文件预览信息
#[derive(Debug, Serialize)]
pub struct BackupPreview {
    pub exported_at: String,
    pub search_history_count: usize,
    pub favorites_count: usize,
    pub learning_records_count: usize,
    pub review_history_count: usize,
}

/// 初始化数据库表
pub fn initialize_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS search_history (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            word        TEXT NOT NULL COLLATE NOCASE UNIQUE,
            searched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS favorites (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    word       TEXT NOT NULL COLLATE NOCASE UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS learning_records (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    word            TEXT NOT NULL COLLATE NOCASE UNIQUE,
    familiarity     INTEGER NOT NULL DEFAULT 1
                    CHECK (familiarity BETWEEN 1 AND 5),
    next_review_at  TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS learning_review_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    word        TEXT NOT NULL COLLATE NOCASE,
    familiarity INTEGER NOT NULL
                CHECK (familiarity BETWEEN 1 AND 5),
    reviewed_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX IF NOT EXISTS idx_learning_review_history_reviewed_at
ON learning_review_history(reviewed_at);

CREATE INDEX IF NOT EXISTS idx_learning_review_history_word
ON learning_review_history(word);

CREATE TABLE IF NOT EXISTS app_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;

    Ok(())
}

/// 添加或更新搜索历史
#[tauri::command]
pub fn add_search_history(word: String, state: State<'_, DatabaseState>) -> Result<(), String> {
    let word = word.trim();

    if word.is_empty() {
        return Ok(());
    }

    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.execute(
        "
        INSERT INTO search_history (word)
        VALUES (?1)
        ON CONFLICT(word)
        DO UPDATE SET searched_at = CURRENT_TIMESTAMP
        ",
        params![word],
    )
    .map_err(|e| format!("保存搜索历史失败：{e}"))?;

    Ok(())
}

/// 获取搜索历史，最新搜索的记录排在前面
#[tauri::command]
pub fn get_search_history(state: State<'_, DatabaseState>) -> Result<Vec<SearchHistory>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    let mut stmt = conn
        .prepare(
            "
            SELECT id, word, searched_at
            FROM search_history
            ORDER BY searched_at DESC, id DESC
            ",
        )
        .map_err(|e| format!("读取搜索历史失败：{e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SearchHistory {
                id: row.get(0)?,
                word: row.get(1)?,
                searched_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询搜索历史失败：{e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取搜索历史记录失败：{e}"))
}

/// 清空搜索历史
#[tauri::command]
pub fn clear_search_history(state: State<'_, DatabaseState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.execute("DELETE FROM search_history", [])
        .map_err(|e| format!("清空搜索历史失败：{e}"))?;

    Ok(())
}

/// 添加收藏
#[tauri::command]
pub fn add_favorite(word: String, state: State<'_, DatabaseState>) -> Result<(), String> {
    let word = word.trim();

    if word.is_empty() {
        return Ok(());
    }

    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.execute(
        "
        INSERT INTO favorites (word)
        VALUES (?1)
        ON CONFLICT(word) DO NOTHING
        ",
        params![word],
    )
    .map_err(|e| format!("添加收藏失败：{e}"))?;

    Ok(())
}

/// 取消收藏
#[tauri::command]
pub fn remove_favorite(word: String, state: State<'_, DatabaseState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.execute(
        "DELETE FROM favorites WHERE word = ?1 COLLATE NOCASE",
        params![word.trim()],
    )
    .map_err(|e| format!("取消收藏失败：{e}"))?;

    Ok(())
}

/// 获取全部收藏
#[tauri::command]
pub fn get_favorites(state: State<'_, DatabaseState>) -> Result<Vec<Favorite>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    let mut stmt = conn
        .prepare(
            "
            SELECT id, word, created_at
            FROM favorites
            ORDER BY created_at DESC, id DESC
            ",
        )
        .map_err(|e| format!("读取收藏失败：{e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Favorite {
                id: row.get(0)?,
                word: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询收藏失败：{e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取收藏记录失败：{e}"))
}

// --------------------------------------------------
// 单词学习记录
// --------------------------------------------------

/// 保存或更新单词学习记录，同时写入复习历史。
#[tauri::command]
pub fn save_learning_record(
    word: String,
    familiarity: i64,
    next_review_at: String,
    state: State<'_, DatabaseState>,
) -> Result<(), String> {
    let word = word.trim();
    let next_review_at = next_review_at.trim();

    if word.is_empty() {
        return Err("单词不能为空".to_string());
    }

    if !(1..=5).contains(&familiarity) {
        return Err("熟悉程度必须在 1 到 5 之间".to_string());
    }

    // 日期格式必须是 YYYY-MM-DD。
    if next_review_at.len() != 10
        || next_review_at.chars().nth(4) != Some('-')
        || next_review_at.chars().nth(7) != Some('-')
    {
        return Err("复习日期格式不正确，应为 YYYY-MM-DD".to_string());
    }

    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    // 使用事务，确保当前学习状态和历史记录同时写入。
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启学习记录事务失败：{e}"))?;

    // 更新当前学习状态。
    tx.execute(
        "
        INSERT INTO learning_records (
            word,
            familiarity,
            next_review_at,
            updated_at
        )
        VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
        ON CONFLICT(word)
        DO UPDATE SET
            familiarity = excluded.familiarity,
            next_review_at = excluded.next_review_at,
            updated_at = CURRENT_TIMESTAMP
        ",
        params![word, familiarity, next_review_at],
    )
    .map_err(|e| format!("保存学习记录失败：{e}"))?;

    // 追加一条复习历史，不覆盖之前的历史记录。
    tx.execute(
        "
        INSERT INTO learning_review_history (
            word,
            familiarity,
            reviewed_at
        )
        VALUES (?1, ?2, datetime('now', 'localtime'))
        ",
        params![word, familiarity],
    )
    .map_err(|e| format!("保存复习历史失败：{e}"))?;

    // 两项操作全部成功后再提交。
    tx.commit()
        .map_err(|e| format!("提交学习记录事务失败：{e}"))?;

    Ok(())
}

/// 获取全部学习记录。
#[tauri::command]
pub fn get_learning_records(
    state: State<'_, DatabaseState>,
) -> Result<Vec<LearningRecord>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    let mut stmt = conn
        .prepare(
            "
            SELECT
                id,
                word,
                familiarity,
                next_review_at,
                updated_at
            FROM learning_records
            ORDER BY next_review_at ASC, updated_at DESC
            ",
        )
        .map_err(|e| format!("读取学习记录失败：{e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(LearningRecord {
                id: row.get(0)?,
                word: row.get(1)?,
                familiarity: row.get(2)?,
                next_review_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| format!("查询学习记录失败：{e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取学习记录失败：{e}"))
}

/// 获取全部复习历史。
#[tauri::command]
pub fn get_review_history(state: State<'_, DatabaseState>) -> Result<Vec<ReviewHistory>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    let mut stmt = conn
        .prepare(
            "
            SELECT
                id,
                word,
                familiarity,
                reviewed_at
            FROM learning_review_history
            ORDER BY reviewed_at DESC, id DESC
            ",
        )
        .map_err(|e| format!("读取复习历史失败：{e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ReviewHistory {
                id: row.get(0)?,
                word: row.get(1)?,
                familiarity: row.get(2)?,
                reviewed_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("查询复习历史失败：{e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取复习历史失败：{e}"))
}

/// 删除指定单词的学习记录。
#[tauri::command]
pub fn remove_learning_record(word: String, state: State<'_, DatabaseState>) -> Result<(), String> {
    let word = word.trim();

    if word.is_empty() {
        return Err("单词不能为空".to_string());
    }

    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.execute(
        "DELETE FROM learning_records WHERE word = ?1 COLLATE NOCASE",
        params![word],
    )
    .map_err(|e| format!("删除学习记录失败：{e}"))?;

    Ok(())
}

/// 批量删除指定单词的学习记录，不删除复习历史。
#[tauri::command]
pub fn remove_learning_records(
    words: Vec<String>,
    state: State<'_, DatabaseState>,
) -> Result<usize, String> {
    let words: Vec<String> = words
        .into_iter()
        .map(|word| word.trim().to_string())
        .filter(|word| !word.is_empty())
        .collect();

    if words.is_empty() {
        return Err("没有选择需要删除的学习记录".to_string());
    }

    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    // 使用事务，确保批量删除操作完整执行。
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启批量删除事务失败：{e}"))?;

    let mut deleted_count = 0;

    for word in &words {
        let affected = tx
            .execute(
                "DELETE FROM learning_records WHERE word = ?1 COLLATE NOCASE",
                params![word],
            )
            .map_err(|e| format!("删除学习记录失败：{e}"))?;

        deleted_count += affected;
    }

    tx.commit()
        .map_err(|e| format!("提交批量删除事务失败：{e}"))?;

    Ok(deleted_count)
}

/// 清空全部复习历史，不删除当前学习记录。
#[tauri::command]
pub fn clear_review_history(state: State<'_, DatabaseState>) -> Result<usize, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    let deleted_count = conn
        .execute("DELETE FROM learning_review_history", [])
        .map_err(|e| format!("清空复习历史失败：{e}"))?;

    Ok(deleted_count)
}

/// 保存上次打开的词典路径
#[tauri::command]
pub fn save_dictionary_path(path: String, state: State<'_, DatabaseState>) -> Result<(), String> {
    let path = path.trim();

    if path.is_empty() {
        return Err("词典路径不能为空".to_string());
    }

    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.execute(
        "
        INSERT INTO app_settings (key, value)
        VALUES ('last_dictionary_path', ?1)
        ON CONFLICT(key)
        DO UPDATE SET value = excluded.value
        ",
        params![path],
    )
    .map_err(|e| format!("保存词典路径失败：{e}"))?;

    Ok(())
}

/// 获取上次打开的词典路径
#[tauri::command]
pub fn get_dictionary_path(state: State<'_, DatabaseState>) -> Result<Option<String>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    conn.query_row(
        "
        SELECT value
        FROM app_settings
        WHERE key = 'last_dictionary_path'
        ",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("读取词典路径失败：{e}"))
}

/// 将前端导出的文件内容写入指定路径。
#[tauri::command]
pub fn write_export_file(path: String, contents: String) -> Result<(), String> {
    let path = path.trim();

    if path.is_empty() {
        return Err("导出文件路径不能为空".to_string());
    }

    std::fs::write(path, contents).map_err(|e| format!("写入导出文件失败：{e}"))?;

    Ok(())
}

// --------------------------------------------------
// 应用数据备份与恢复
// --------------------------------------------------

/// 备份文件格式标识。
const BACKUP_FORMAT: &str = "my-dictionary-backup";

/// 当前备份文件格式版本。
const BACKUP_SCHEMA_VERSION: u32 = 1;

/// 允许导入的最大备份文件大小：50 MB。
const MAX_BACKUP_SIZE: usize = 50 * 1024 * 1024;

/// 检查日期是否符合 YYYY-MM-DD 格式。
fn is_valid_date_string(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| index == 4 || index == 7 || ch.is_ascii_digit())
}

/// 导出全部应用数据为 JSON。
#[tauri::command]
pub fn export_app_backup(state: State<'_, DatabaseState>) -> Result<String, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    // 搜索历史。
    let mut stmt = conn
        .prepare(
            "
            SELECT id, word, searched_at
            FROM search_history
            ORDER BY id ASC
            ",
        )
        .map_err(|e| format!("读取搜索历史失败：{e}"))?;

    let search_history = stmt
        .query_map([], |row| {
            Ok(SearchHistory {
                id: row.get(0)?,
                word: row.get(1)?,
                searched_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询搜索历史失败：{e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取搜索历史失败：{e}"))?;

    // 收藏。
    let mut stmt = conn
        .prepare(
            "
            SELECT id, word, created_at
            FROM favorites
            ORDER BY id ASC
            ",
        )
        .map_err(|e| format!("读取收藏失败：{e}"))?;

    let favorites = stmt
        .query_map([], |row| {
            Ok(Favorite {
                id: row.get(0)?,
                word: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询收藏失败：{e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取收藏失败：{e}"))?;

    // 当前学习记录。
    let mut stmt = conn
        .prepare(
            "
            SELECT id, word, familiarity, next_review_at, updated_at
            FROM learning_records
            ORDER BY id ASC
            ",
        )
        .map_err(|e| format!("读取学习记录失败：{e}"))?;

    let learning_records = stmt
        .query_map([], |row| {
            Ok(LearningRecord {
                id: row.get(0)?,
                word: row.get(1)?,
                familiarity: row.get(2)?,
                next_review_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| format!("查询学习记录失败：{e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取学习记录失败：{e}"))?;

    // 历史复习记录。
    let mut stmt = conn
        .prepare(
            "
            SELECT id, word, familiarity, reviewed_at
            FROM learning_review_history
            ORDER BY id ASC
            ",
        )
        .map_err(|e| format!("读取复习历史失败：{e}"))?;

    let review_history = stmt
        .query_map([], |row| {
            Ok(ReviewHistory {
                id: row.get(0)?,
                word: row.get(1)?,
                familiarity: row.get(2)?,
                reviewed_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("查询复习历史失败：{e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取复习历史失败：{e}"))?;

    // 获取本地时间。
    let exported_at: String = conn
        .query_row("SELECT datetime('now', 'localtime')", [], |row| row.get(0))
        .map_err(|e| format!("获取备份时间失败：{e}"))?;

    let backup = AppBackup {
        format: BACKUP_FORMAT.to_string(),
        schema_version: BACKUP_SCHEMA_VERSION,
        exported_at,
        search_history,
        favorites,
        learning_records,
        review_history,
    };

    serde_json::to_string_pretty(&backup).map_err(|e| format!("生成 JSON 备份失败：{e}"))
}

/// 检查备份文件，并返回其中的数据统计信息。
#[tauri::command]
pub fn inspect_app_backup(path: String) -> Result<BackupPreview, String> {
    let path = path.trim();

    if path.is_empty() {
        return Err("备份文件路径不能为空".to_string());
    }

    // 检查文件大小。
    let metadata = std::fs::metadata(path).map_err(|e| format!("读取备份文件信息失败：{e}"))?;

    if metadata.len() > MAX_BACKUP_SIZE as u64 {
        return Err("备份文件超过 50 MB，无法读取。".to_string());
    }

    // 读取 JSON 文件。
    let contents = std::fs::read_to_string(path).map_err(|e| format!("读取备份文件失败：{e}"))?;

    let backup: AppBackup =
        serde_json::from_str(&contents).map_err(|e| format!("备份文件格式不正确：{e}"))?;

    // 检查备份类型。
    if backup.format != BACKUP_FORMAT {
        return Err("这不是 My Dictionary 的备份文件。".to_string());
    }

    // 检查备份版本。
    if backup.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(format!(
            "不支持的备份版本：{}，当前支持版本：{}",
            backup.schema_version, BACKUP_SCHEMA_VERSION
        ));
    }

    if backup.exported_at.trim().is_empty() {
        return Err("备份文件缺少生成时间。".to_string());
    }

    Ok(BackupPreview {
        exported_at: backup.exported_at,
        search_history_count: backup.search_history.len(),
        favorites_count: backup.favorites.len(),
        learning_records_count: backup.learning_records.len(),
        review_history_count: backup.review_history.len(),
    })
}

/// 从 JSON 文件恢复应用数据。
#[tauri::command]
pub fn restore_app_backup(path: String, state: State<'_, DatabaseState>) -> Result<(), String> {
    let path = path.trim();

    if path.is_empty() {
        return Err("备份文件路径不能为空".to_string());
    }

    // 检查文件大小。
    let metadata = std::fs::metadata(path).map_err(|e| format!("读取备份文件信息失败：{e}"))?;

    if metadata.len() > MAX_BACKUP_SIZE as u64 {
        return Err("备份文件超过 50 MB，无法导入。".to_string());
    }

    // 读取 JSON 文件。
    let contents = std::fs::read_to_string(path).map_err(|e| format!("读取备份文件失败：{e}"))?;

    let backup: AppBackup =
        serde_json::from_str(&contents).map_err(|e| format!("备份文件格式不正确：{e}"))?;

    // 检查备份类型。
    if backup.format != BACKUP_FORMAT {
        return Err("这不是 My Dictionary 的备份文件。".to_string());
    }

    // 检查备份版本。
    if backup.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(format!(
            "不支持的备份版本：{}，当前支持版本：{}",
            backup.schema_version, BACKUP_SCHEMA_VERSION
        ));
    }

    if backup.exported_at.trim().is_empty() {
        return Err("备份文件缺少生成时间。".to_string());
    }

    // 检查搜索历史。
    for item in &backup.search_history {
        if item.id <= 0 || item.word.trim().is_empty() || item.searched_at.trim().is_empty() {
            return Err("搜索历史中存在无效记录。".to_string());
        }
    }

    // 检查收藏。
    for item in &backup.favorites {
        if item.id <= 0 || item.word.trim().is_empty() || item.created_at.trim().is_empty() {
            return Err("收藏记录中存在无效记录。".to_string());
        }
    }

    // 检查学习记录。
    for item in &backup.learning_records {
        if item.id <= 0
            || item.word.trim().is_empty()
            || !(1..=5).contains(&item.familiarity)
            || !is_valid_date_string(&item.next_review_at)
            || item.updated_at.trim().is_empty()
        {
            return Err("学习记录中存在无效数据。".to_string());
        }
    }

    // 检查复习历史。
    for item in &backup.review_history {
        if item.id <= 0
            || item.word.trim().is_empty()
            || !(1..=5).contains(&item.familiarity)
            || item.reviewed_at.trim().is_empty()
        {
            return Err("复习历史中存在无效数据。".to_string());
        }
    }

    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁定失败：{e}"))?;

    // 开启事务。
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启恢复事务失败：{e}"))?;

    // 清空当前四类数据。
    tx.execute("DELETE FROM search_history", [])
        .map_err(|e| format!("清空搜索历史失败：{e}"))?;

    tx.execute("DELETE FROM favorites", [])
        .map_err(|e| format!("清空收藏失败：{e}"))?;

    tx.execute("DELETE FROM learning_records", [])
        .map_err(|e| format!("清空学习记录失败：{e}"))?;

    tx.execute("DELETE FROM learning_review_history", [])
        .map_err(|e| format!("清空复习历史失败：{e}"))?;

    // 恢复搜索历史。
    for item in &backup.search_history {
        tx.execute(
            "
            INSERT INTO search_history (id, word, searched_at)
            VALUES (?1, ?2, ?3)
            ",
            params![item.id, item.word, item.searched_at],
        )
        .map_err(|e| format!("恢复搜索历史失败：{e}"))?;
    }

    // 恢复收藏。
    for item in &backup.favorites {
        tx.execute(
            "
            INSERT INTO favorites (id, word, created_at)
            VALUES (?1, ?2, ?3)
            ",
            params![item.id, item.word, item.created_at],
        )
        .map_err(|e| format!("恢复收藏失败：{e}"))?;
    }

    // 恢复学习记录。
    for item in &backup.learning_records {
        tx.execute(
            "
            INSERT INTO learning_records (
                id,
                word,
                familiarity,
                next_review_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                item.id,
                item.word,
                item.familiarity,
                item.next_review_at,
                item.updated_at
            ],
        )
        .map_err(|e| format!("恢复学习记录失败：{e}"))?;
    }

    // 恢复复习历史。
    for item in &backup.review_history {
        tx.execute(
            "
            INSERT INTO learning_review_history (
                id,
                word,
                familiarity,
                reviewed_at
            )
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![item.id, item.word, item.familiarity, item.reviewed_at],
        )
        .map_err(|e| format!("恢复复习历史失败：{e}"))?;
    }

    // 所有数据恢复成功后，统一提交。
    tx.commit()
        .map_err(|e| format!("提交数据恢复事务失败：{e}"))?;

    Ok(())
}
