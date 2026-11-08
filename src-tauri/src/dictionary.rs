use mdictlib::MdxFile;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

/// 当前词典管理状态。
///
/// 所有已经打开的 MDX 词典都保存在 dictionaries 中。
/// active_path 表示当前正在使用的词典路径。
pub struct DictionaryState {
    pub inner: Mutex<DictionaryCollection>,
}

/// 多词典集合。
pub struct DictionaryCollection {
    pub dictionaries: Vec<LoadedDictionary>,
    pub active_path: Option<String>,
}

/// 已经加载的词典。
pub struct LoadedDictionary {
    pub file: MdxFile,
    pub info: DictionaryInfo,
}

/// 词典基本信息。
#[derive(Debug, Clone, Serialize)]
pub struct DictionaryInfo {
    pub name: String,
    pub path: String,
    pub entry_count: u64,

    /// 是否参与搜索。
    pub enabled: bool,
}

/// 单词查询结果。
#[derive(Debug, Serialize)]
pub struct WordResult {
    pub headword: String,
    pub definition: String,
}

/// 打开本地 MDX 词典。
///
/// 如果词典已经加载，则直接切换，不重复加载。
#[tauri::command]
pub fn open_dictionary(
    path: String,
    state: State<'_, DictionaryState>,
) -> Result<DictionaryInfo, String> {
    let dictionary_path = PathBuf::from(&path);

    if !dictionary_path.exists() {
        return Err("词典文件不存在".to_string());
    }

    if dictionary_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| !ext.eq_ignore_ascii_case("mdx"))
        .unwrap_or(true)
    {
        return Err("请选择 MDX 格式的词典文件".to_string());
    }

    let normalized_path = std::fs::canonicalize(&dictionary_path)
        .map_err(|e| format!("无法解析词典路径：{e}"))?
        .to_string_lossy()
        .to_string();

    let mut collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    // 已经加载过的词典不重复打开。
    if let Some(existing) = collection
        .dictionaries
        .iter_mut()
        .find(|item| item.info.path.eq_ignore_ascii_case(&normalized_path))
    {
        // 用户通过添加词典操作重新打开时，自动启用该词典。
        existing.info.enabled = true;

        let info = existing.info.clone();
        collection.active_path = Some(normalized_path);

        return Ok(info);
    }

    // 打开 MDX 文件。
    let file = MdxFile::open(&normalized_path).map_err(|e| format!("加载词典失败：{e}"))?;

    let name = dictionary_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名词典")
        .to_string();

    let info = DictionaryInfo {
        name,
        path: normalized_path.clone(),
        entry_count: file.len(),
        enabled: true,
    };

    collection.dictionaries.push(LoadedDictionary {
        file,
        info: info.clone(),
    });

    // 新加载的词典自动成为当前词典。
    collection.active_path = Some(normalized_path);

    Ok(info)
}

/// 获取所有已经加载的词典。
#[tauri::command]
pub fn get_dictionaries(state: State<'_, DictionaryState>) -> Result<Vec<DictionaryInfo>, String> {
    let collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    Ok(collection
        .dictionaries
        .iter()
        .map(|item| item.info.clone())
        .collect())
}

/// 切换当前使用的词典。
#[tauri::command]
pub fn switch_dictionary(
    path: String,
    state: State<'_, DictionaryState>,
) -> Result<DictionaryInfo, String> {
    let mut collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    let dictionary = collection
        .dictionaries
        .iter()
        .find(|item| item.info.path.eq_ignore_ascii_case(&path))
        .ok_or_else(|| "该词典尚未加载".to_string())?;

    if !dictionary.info.enabled {
        return Err("该词典已经停用，无法切换。".to_string());
    }

    let info = dictionary.info.clone();
    collection.active_path = Some(info.path.clone());

    Ok(info)
}

/// 启用或停用指定词典。
///
/// 返回操作完成后当前正在使用的词典信息。
#[tauri::command]
pub fn set_dictionary_enabled(
    path: String,
    enabled: bool,
    state: State<'_, DictionaryState>,
) -> Result<Option<DictionaryInfo>, String> {
    let mut collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    // 查找目标词典。
    let dictionary = collection
        .dictionaries
        .iter_mut()
        .find(|item| item.info.path.eq_ignore_ascii_case(&path))
        .ok_or_else(|| "没有找到指定词典".to_string())?;

    dictionary.info.enabled = enabled;

    // 如果启用了一本词典，但当前没有活动词典，
    // 则自动将它设为当前词典。
    if enabled && collection.active_path.is_none() {
        collection.active_path = Some(path.clone());
    }

    // 如果停用的是当前词典，则自动选择其他启用的词典。
    if !enabled
        && collection
            .active_path
            .as_deref()
            .map(|active| active.eq_ignore_ascii_case(&path))
            .unwrap_or(false)
    {
        collection.active_path = collection
            .dictionaries
            .iter()
            .find(|item| item.info.enabled)
            .map(|item| item.info.path.clone());
    }

    // 返回当前活动词典。
    let active_dictionary = collection.active_path.as_ref().and_then(|active_path| {
        collection
            .dictionaries
            .iter()
            .find(|item| item.info.path.eq_ignore_ascii_case(active_path))
            .map(|item| item.info.clone())
    });

    Ok(active_dictionary)
}

/// 从当前词典库中移除指定词典。
///
/// 仅移除内存中的词典，不会删除硬盘上的 MDX 文件。
#[tauri::command]
pub fn remove_dictionary(path: String, state: State<'_, DictionaryState>) -> Result<(), String> {
    let mut collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    let original_len = collection.dictionaries.len();

    collection
        .dictionaries
        .retain(|item| !item.info.path.eq_ignore_ascii_case(&path));

    if collection.dictionaries.len() == original_len {
        return Err("没有找到要移除的词典".to_string());
    }

    // 如果移除的是当前词典，自动选择剩余词典中的第一部。
    if collection
        .active_path
        .as_deref()
        .map(|active| active.eq_ignore_ascii_case(&path))
        .unwrap_or(false)
    {
        collection.active_path = collection
            .dictionaries
            .iter()
            .find(|item| item.info.enabled)
            .map(|item| item.info.path.clone());
    }

    Ok(())
}

/// 获取当前正在使用的词典。
fn get_active_dictionary(collection: &DictionaryCollection) -> Result<&LoadedDictionary, String> {
    let active_path = collection
        .active_path
        .as_ref()
        .ok_or_else(|| "请先选择一本已启用的词典".to_string())?;

    let dictionary = collection
        .dictionaries
        .iter()
        .find(|item| &item.info.path == active_path)
        .ok_or_else(|| "当前词典不存在".to_string())?;

    if !dictionary.info.enabled {
        return Err("当前词典已经停用，请选择一本已启用的词典".to_string());
    }

    Ok(dictionary)
}

/// 搜索英文单词。
#[tauri::command]
pub fn search_word(
    word: String,
    state: State<'_, DictionaryState>,
) -> Result<Option<WordResult>, String> {
    let word = word.trim();

    if word.is_empty() {
        return Ok(None);
    }

    let collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    let dictionary = get_active_dictionary(&collection)?;

    let entry = dictionary
        .file
        .lookup(word)
        .map_err(|e| format!("查询词条失败：{e}"))?;

    match entry {
        Some(entry) => Ok(Some(WordResult {
            headword: entry.key().to_string(),
            definition: entry.text().to_string(),
        })),
        None => Ok(None),
    }
}

/// 根据前缀搜索词典中的单词。
///
/// 例如输入 app，返回以 app 开头的词条。
#[tauri::command]
pub fn suggest_words(
    prefix: String,
    limit: Option<usize>,
    state: State<'_, DictionaryState>,
) -> Result<Vec<String>, String> {
    let prefix = prefix.trim();

    if prefix.is_empty() {
        return Ok(Vec::new());
    }

    let collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    let dictionary = get_active_dictionary(&collection)?;

    let limit = limit.unwrap_or(20).clamp(1, 100);

    let entries = dictionary
        .file
        .prefix_keys(prefix, limit)
        .map_err(|e| format!("前缀搜索失败：{e}"))?;

    let words = entries
        .into_iter()
        .map(|entry| entry.key().to_string())
        .collect();

    Ok(words)
}

// --------------------------------------------------
// 跨词典搜索建议
// --------------------------------------------------

/// 在所有已加载词典中搜索指定前缀的单词。
///
/// 搜索结果会进行大小写不敏感的去重，
/// 并按照字母顺序排序。
#[tauri::command]
pub fn suggest_words_all_dictionaries(
    prefix: String,
    limit: Option<usize>,
    state: State<'_, DictionaryState>,
) -> Result<Vec<String>, String> {
    let prefix = prefix.trim();

    if prefix.is_empty() {
        return Ok(Vec::new());
    }

    let collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    if !collection.dictionaries.iter().any(|item| item.info.enabled) {
        return Err("请先启用至少一本 MDX 词典".to_string());
    }

    let limit = limit.unwrap_or(20).clamp(1, 100);

    let mut words: Vec<String> = Vec::new();

    // 只遍历已经启用的词典。
    for dictionary in collection
        .dictionaries
        .iter()
        .filter(|item| item.info.enabled)
    {
        let entries = dictionary
            .file
            .prefix_keys(prefix, limit)
            .map_err(|e| format!("词典「{}」前缀搜索失败：{e}", dictionary.info.name))?;

        for entry in entries {
            words.push(entry.key().to_string());
        }
    }

    // 按照小写形式排序。
    words.sort_by_key(|word| word.to_lowercase());

    // 删除大小写不同但实际相同的词头。
    words.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    // 限制最终返回的结果数量。
    words.truncate(limit);

    Ok(words)
}

// --------------------------------------------------
// 跨词典搜索
// --------------------------------------------------

use std::ops::ControlFlow;

/// 跨词典搜索结果。
#[derive(Debug, Serialize)]
pub struct DictionarySearchResult {
    pub headword: String,
    pub definition: String,

    /// 来源词典名称。
    pub dictionary_name: String,

    /// 来源词典路径。
    pub dictionary_path: String,

    /// 匹配类型：exact、prefix、fuzzy。
    pub match_type: String,

    /// 编辑距离，仅模糊匹配时有值。
    pub distance: Option<usize>,
}

/// 计算两个字符串的 Levenshtein 编辑距离。
///
/// 如果编辑距离超过 max_distance，则返回 None。
fn levenshtein_distance(left: &str, right: &str, max_distance: usize) -> Option<usize> {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();

    if left.len().abs_diff(right.len()) > max_distance {
        return None;
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();

    let mut current = vec![0; right.len() + 1];

    for (i, left_char) in left.iter().enumerate() {
        current[0] = i + 1;

        let mut row_min = current[0];

        for (j, right_char) in right.iter().enumerate() {
            let cost = if left_char == right_char { 0 } else { 1 };

            let deletion = previous[j + 1] + 1;
            let insertion = current[j] + 1;
            let substitution = previous[j] + cost;

            current[j + 1] = deletion.min(insertion).min(substitution);

            row_min = row_min.min(current[j + 1]);
        }

        if row_min > max_distance {
            return None;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    let distance = previous[right.len()];

    if distance <= max_distance {
        Some(distance)
    } else {
        None
    }
}

/// 将 MDX 词条转换为跨词典搜索结果。
fn make_dictionary_result(
    entry: &mdictlib::MdxEntry,
    info: &DictionaryInfo,
    match_type: &str,
    distance: Option<usize>,
) -> DictionarySearchResult {
    DictionarySearchResult {
        headword: entry.key().to_string(),
        definition: entry.text().to_string(),
        dictionary_name: info.name.clone(),
        dictionary_path: info.path.clone(),
        match_type: match_type.to_string(),
        distance,
    }
}

/// 在所有已加载词典中搜索单词。
///
/// 搜索优先级：
/// 1. 精确匹配
/// 2. 前缀匹配
/// 3. 编辑距离模糊匹配
#[tauri::command]
pub fn search_all_dictionaries(
    word: String,
    state: State<'_, DictionaryState>,
) -> Result<Vec<DictionarySearchResult>, String> {
    let query = word.trim();

    if query.is_empty() {
        return Ok(Vec::new());
    }

    let collection = state
        .inner
        .lock()
        .map_err(|e| format!("词典状态锁定失败：{e}"))?;

    if !collection.dictionaries.iter().any(|item| item.info.enabled) {
        return Err("请先启用至少一本 MDX 词典".to_string());
    }

    let normalized_query = query.to_lowercase();

    // --------------------------------------------------
    // 第一优先级：精确匹配
    // --------------------------------------------------

    let mut exact_results = Vec::new();

    for dictionary in collection
        .dictionaries
        .iter()
        .filter(|item| item.info.enabled)
    {
        let entry = dictionary
            .file
            .lookup(query)
            .map_err(|e| format!("查询词典「{}」失败：{e}", dictionary.info.name))?;

        if let Some(entry) = entry {
            exact_results.push(make_dictionary_result(
                &entry,
                &dictionary.info,
                "exact",
                None,
            ));
        }
    }

    if !exact_results.is_empty() {
        return Ok(exact_results);
    }

    // --------------------------------------------------
    // 第二优先级：前缀匹配
    // --------------------------------------------------

    let mut prefix_results = Vec::new();

    for dictionary in collection
        .dictionaries
        .iter()
        .filter(|item| item.info.enabled)
    {
        let entries = dictionary
            .file
            .prefix_keys(query, 8)
            .map_err(|e| format!("词典「{}」前缀搜索失败：{e}", dictionary.info.name))?;

        for key in entries {
            let entry = dictionary
                .file
                .entry_at(key.ordinal())
                .map_err(|e| format!("读取词条失败：{e}"))?;

            if let Some(entry) = entry {
                prefix_results.push(make_dictionary_result(
                    &entry,
                    &dictionary.info,
                    "prefix",
                    None,
                ));
            }
        }
    }

    if !prefix_results.is_empty() {
        return Ok(prefix_results);
    }

    // --------------------------------------------------
    // 第三优先级：模糊匹配
    // --------------------------------------------------

    // 太短的字符串不执行模糊匹配。
    if normalized_query.chars().count() < 3 {
        return Ok(Vec::new());
    }

    // 根据查询词长度决定最大编辑距离。
    let max_distance = match normalized_query.chars().count() {
        0..=5 => 1,
        6..=10 => 2,
        _ => 3,
    };

    // 每本词典最多保留 5 个模糊匹配结果。
    const FUZZY_LIMIT: usize = 5;

    let mut fuzzy_results = Vec::new();

    for dictionary in collection
        .dictionaries
        .iter()
        .filter(|item| item.info.enabled)
    {
        // 保存候选词条的编辑距离和物理序号。
        let mut candidates: Vec<(usize, mdictlib::KeyOrdinal)> = Vec::new();

        dictionary
            .file
            .scan_normalized_keys(|ordinal, normalized_key| {
                if let Some(distance) =
                    levenshtein_distance(&normalized_query, normalized_key, max_distance)
                {
                    candidates.push((distance, ordinal));

                    candidates.sort_by_key(|item| item.0);

                    if candidates.len() > FUZZY_LIMIT {
                        candidates.pop();
                    }
                }

                ControlFlow::Continue(())
            })
            .map_err(|e| format!("词典「{}」模糊搜索失败：{e}", dictionary.info.name))?;

        // 根据编辑距离从小到大读取释义。
        for (distance, ordinal) in candidates {
            let entry = dictionary
                .file
                .entry_at(ordinal)
                .map_err(|e| format!("读取模糊匹配词条失败：{e}"))?;

            if let Some(entry) = entry {
                fuzzy_results.push(make_dictionary_result(
                    &entry,
                    &dictionary.info,
                    "fuzzy",
                    Some(distance),
                ));
            }
        }
    }

    fuzzy_results.sort_by_key(|item| item.distance.unwrap_or(usize::MAX));

    Ok(fuzzy_results)
}
