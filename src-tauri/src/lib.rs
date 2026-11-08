mod database;
mod dictionary;

use std::sync::Mutex;

use tauri::Manager;

use crate::database::{
    DatabaseState,
    add_favorite,
    add_search_history,
    clear_review_history,
    clear_search_history,
    export_app_backup,
    get_dictionary_path,
    get_favorites,
    get_learning_records,
    get_review_history,
    get_search_history,
    initialize_database,
    inspect_app_backup,
    remove_favorite,
    remove_learning_record,
    remove_learning_records,
    restore_app_backup,
    save_dictionary_path,
    save_learning_record,
    write_export_file,
};

use crate::dictionary::{
    DictionaryCollection,
    DictionaryState,
    get_dictionaries,
    open_dictionary,
    remove_dictionary,
    search_all_dictionaries,
    search_word,
    set_dictionary_enabled,
    suggest_words,
    suggest_words_all_dictionaries,
    switch_dictionary,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // 初始化数据库和应用状态。
        .setup(|app| {
            // 获取应用数据目录。
            let app_data_dir = app.path().app_data_dir()?;

            // 确保目录存在。
            std::fs::create_dir_all(&app_data_dir)?;

            // 创建或打开 SQLite 数据库。
            let db_path = app_data_dir.join("dictionary.db");

            let conn = rusqlite::Connection::open(db_path)?;

            // 初始化数据库表。
            initialize_database(&conn)?;

            // 注册 SQLite 数据库状态。
            app.manage(DatabaseState {
                conn: Mutex::new(conn),
            });

            // 注册 MDX 多词典状态。
            app.manage(DictionaryState {
                inner: Mutex::new(DictionaryCollection {
                    dictionaries: Vec::new(),
                    active_path: None,
                }),
            });

            Ok(())
        })
        // 注册所有前端可调用的命令。
        .invoke_handler(tauri::generate_handler![
            // 词典管理。
            open_dictionary,
            get_dictionaries,
            switch_dictionary,
            set_dictionary_enabled,
            remove_dictionary,
            // 单词查询。
            search_word,
            suggest_words,
            suggest_words_all_dictionaries,
            search_all_dictionaries,
            // 搜索历史。
            add_search_history,
            get_search_history,
            clear_search_history,
            // 收藏。
            add_favorite,
            remove_favorite,
            get_favorites,
            // 单词学习。
            save_learning_record,
            get_learning_records,
            get_review_history,
            remove_learning_record,
            remove_learning_records,
            clear_review_history,
            // 文件导出与数据备份。
            write_export_file,
            export_app_backup,
            inspect_app_backup,
            restore_app_backup,
            // 上次使用的词典路径。
            save_dictionary_path,
            get_dictionary_path,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
