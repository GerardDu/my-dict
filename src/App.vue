<script setup lang="ts">
import { computed, onMounted, onUnmounted, nextTick, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import DOMPurify from 'dompurify'
import { getCurrentWindow } from '@tauri-apps/api/window'

import {
  BookOpen,
  Search,
  Clock3,
  CalendarCheck,
  Bookmark,
  Settings,
  X,
  Star,
  Trash2,
  Check,
  AlertCircle,
  RefreshCw,
  Languages,
  ChevronRight,
  Library,
  Plus,
  GraduationCap,
  Download,
  PanelLeftClose,
  PanelLeftOpen,
  Power,
  MoreVertical,
Minus,
Maximize2,
} from 'lucide-vue-next'

const DICTIONARY_LIBRARY_KEY = 'my-dict-dictionary-library'

// 保存已经停用的词典路径。
const DISABLED_DICTIONARIES_KEY = 'my-dict-disabled-dictionaries'



// 当前 Tauri 窗口实例。
const appWindow = getCurrentWindow()

// 最小化窗口。
async function minimizeWindow() {
  try {
    await appWindow.minimize()
  } catch (error) {
    console.error('窗口最小化失败：', error)
  }
}

// 最大化或恢复窗口。
async function toggleWindowMaximize() {
  try {
    await appWindow.toggleMaximize()
  } catch (error) {
    console.error('切换窗口大小失败：', error)
  }
}

// 关闭窗口。
async function closeWindow() {
  try {
    await appWindow.close()
  } catch (error) {
    console.error('关闭窗口失败：', error)
  }
}

// 打开设置页面。
function openSettings() {
  switchPage('settings')
}


function saveDictionaryLibrary() {
  const paths = dictionaries.value.map((item) => item.path)

  localStorage.setItem(DICTIONARY_LIBRARY_KEY, JSON.stringify(paths))
}

// 仅用于界面显示，去掉 Windows 扩展路径前缀。
// 不修改实际传递给 Rust 的词典路径。
function displayDictionaryPath(path: string) {
  const uncPrefix = '\\\\?\\UNC\\'
  const windowsPrefix = '\\\\?\\'

  if (path.startsWith(uncPrefix)) {
    return `\\\\${path.slice(uncPrefix.length)}`
  }

  if (path.startsWith(windowsPrefix)) {
    return path.slice(windowsPrefix.length)
  }

  return path
}

// 保存停用词典路径。
function saveDisabledDictionaries() {
  const paths = dictionaries.value
    .filter((item) => !item.enabled)
    .map((item) => item.path)

  localStorage.setItem(DISABLED_DICTIONARIES_KEY, JSON.stringify(paths))
}

// --------------------------------------------------
// 类型定义
// --------------------------------------------------

type PageName =
  | 'search'
  | 'history'
  | 'favorites'
  | 'learning'
  | 'review-history'
  | 'settings'
  | 'dictionaries'

interface DictionaryInfo {
  name: string
  path: string
  entry_count: number
  enabled: boolean
}

const dictionaries = ref<DictionaryInfo[]>([])
const activeDictionaryPath = ref('')

// 当前所有已启用的词典。
const enabledDictionaries = computed(() => {
  return dictionaries.value.filter((item) => item.enabled)
})

interface WordResult {
  headword: string
  definition: string
}

interface DictionarySearchResult {
  headword: string
  definition: string
  dictionary_name: string
  dictionary_path: string
  match_type: 'exact' | 'prefix' | 'fuzzy'
  distance: number | null
}

// 相同词头的多词典搜索结果分组。
interface DictionaryResultGroup {
  headword: string
  results: DictionarySearchResult[]
}

interface SearchHistory {
  id: number
  word: string
  searched_at: string
}

interface Favorite {
  id: number
  word: string
  created_at: string
}

// 单词学习记录
interface LearningRecord {
  id: number
  word: string
  familiarity: number
  next_review_at: string
  updated_at: string
}

// 单词复习历史
interface ReviewHistory {
  id: number
  word: string
  familiarity: number
  reviewed_at: string
}

// 备份文件预览信息
interface BackupPreview {
  exported_at: string
  search_history_count: number
  favorites_count: number
  learning_records_count: number
  review_history_count: number
}

interface SearchSuggestion {
  word: string
  source: '词典' | '历史' | '收藏'
}

// --------------------------------------------------
// 页面状态
// --------------------------------------------------

const currentPage = ref<PageName>('search')

// 左侧导航栏是否收缩。
const isSidebarCollapsed = ref(false)

// 记录上一次窗口宽度所处的状态，避免普通窗口缩放覆盖用户手动设置。
let previousSidebarBreakpoint: boolean | null = null

function syncSidebarLayout() {
  const isNarrow = window.innerWidth <= 1050

  if (previousSidebarBreakpoint !== isNarrow) {
    isSidebarCollapsed.value = isNarrow
    previousSidebarBreakpoint = isNarrow
  }
}

// 手动收缩或展开导航栏。
function toggleSidebar() {
  isSidebarCollapsed.value = !isSidebarCollapsed.value
}

// 搜索框
const searchText = ref('')
const isSearching = ref(false)

// 搜索输入框 DOM
const searchInput = ref<HTMLInputElement | null>(null)

// 搜索建议
const suggestions = ref<SearchSuggestion[]>([])
const isSuggestionOpen = ref(false)
const selectedSuggestionIndex = ref(-1)

// 搜索建议请求控制
let suggestionTimer: ReturnType<typeof setTimeout> | undefined
let suggestionRequestId = 0

// 词典状态
const dictionaryInfo = ref<DictionaryInfo | null>(null)
const dictionaryPath = ref('')

const activeDictionary = computed(() => {
  return (
    dictionaries.value.find(
      (item) => item.path === activeDictionaryPath.value,
    ) ?? null
  )
})

// 搜索结果
const searchResults = ref<DictionarySearchResult[]>([])
const currentDefinition = ref<WordResult | null>(null)

// 当前正在阅读的词典路径
const currentDictionaryPath = ref('')

// 每个词头当前选中的词典路径。
// 使用词头作为键，使不同词头的选择互不影响。
const selectedDictionaryByHeadword = ref<Record<string, string>>({})

// 搜索范围：全部词典 / 当前词典
const searchScope = ref<'all' | 'current'>('all')

// 释义窗口中的词典来源筛选。
const selectedDictionaryFilter = ref('all')

// 搜索结果匹配类型筛选。
// all、exact、prefix、fuzzy 分别表示全部、精确、前缀和模糊匹配。
const selectedMatchType = ref<'all' | 'exact' | 'prefix' | 'fuzzy'>('all')

// 匹配类型的排序权重。
const matchTypeOrder: Record<string, number> = {
  exact: 0,
  prefix: 1,
  fuzzy: 2,
}

// 按匹配类型和编辑距离排序。
const sortedSearchResults = computed(() => {
  return [...searchResults.value].sort((a, b) => {
    const typeDifference =
      (matchTypeOrder[a.match_type] ?? 99) -
      (matchTypeOrder[b.match_type] ?? 99)

    if (typeDifference !== 0) {
      return typeDifference
    }

    const distanceDifference = (a.distance ?? 0) - (b.distance ?? 0)

    if (distanceDifference !== 0) {
      return distanceDifference
    }

    return a.headword.localeCompare(b.headword)
  })
})

// 根据词典来源筛选结果。
// 根据词典来源和匹配类型筛选结果。
const filteredSearchResults = computed(() => {
  return sortedSearchResults.value.filter((item) => {
    // 词典来源筛选。
    const dictionaryMatched =
      selectedDictionaryFilter.value === 'all' ||
      item.dictionary_path === selectedDictionaryFilter.value

    // 匹配类型筛选。
    const matchTypeMatched =
      selectedMatchType.value === 'all' ||
      item.match_type === selectedMatchType.value

    return dictionaryMatched && matchTypeMatched
  })
})

// 将相同词头的结果合并为一个分组。
// 每个分组仍然保留来自不同词典的独立释义。
const groupedSearchResults = computed<DictionaryResultGroup[]>(() => {
  const groups = new Map<string, DictionaryResultGroup>()

  for (const item of filteredSearchResults.value) {
    const normalizedHeadword = item.headword.toLocaleLowerCase()

    let group = groups.get(normalizedHeadword)

    if (!group) {
      group = {
        headword: item.headword,
        results: [],
      }

      groups.set(normalizedHeadword, group)
    }

    group.results.push(item)
  }

  return Array.from(groups.values())
})

// 获取某个词头当前选中的词典释义。
function getSelectedResult(
  group: DictionaryResultGroup,
): DictionarySearchResult | undefined {
  const selectedPath =
    selectedDictionaryByHeadword.value[group.headword.toLocaleLowerCase()]

  return (
    group.results.find((item) => item.dictionary_path === selectedPath) ??
    group.results[0]
  )
}

// --------------------------------------------------
// 筛选结果变化时，同步当前单词和词典
// --------------------------------------------------

watch(groupedSearchResults, (groups) => {
  // 筛选后没有任何结果。
  if (groups.length === 0) {
    currentDefinition.value = null
    currentDictionaryPath.value = ''
    return
  }

  // 尽量保留当前正在查看的词头。
  const currentHeadword = currentDefinition.value?.headword.toLocaleLowerCase()

  let currentGroup = groups.find(
    (group) => group.headword.toLocaleLowerCase() === currentHeadword,
  )

  // 如果当前词头已被筛选掉，则自动选择第一个可见词头。
  if (!currentGroup) {
    currentGroup = groups[0]
  }

  const selectedResult = getSelectedResult(currentGroup)

  if (!selectedResult) {
    currentDefinition.value = null
    currentDictionaryPath.value = ''
    return
  }

  // 同步当前单词和词典来源。
  currentDefinition.value = {
    headword: selectedResult.headword,
    definition: selectedResult.definition,
  }

  currentDictionaryPath.value = selectedResult.dictionary_path
})

// 切换某个词头对应的词典释义。
function selectGroupDictionary(
  group: DictionaryResultGroup,
  item: DictionarySearchResult,
) {
  const key = group.headword.toLocaleLowerCase()

  selectedDictionaryByHeadword.value = {
    ...selectedDictionaryByHeadword.value,
    [key]: item.dictionary_path,
  }

  // 同步当前词头，确保收藏操作对应正确的单词。
  currentDefinition.value = {
    headword: item.headword,
    definition: item.definition,
  }

  currentDictionaryPath.value = item.dictionary_path
}

// 释义模态窗口
const isDefinitionOpen = ref(false)

// 搜索历史、收藏和学习记录
const historyList = ref<SearchHistory[]>([])
const favoriteList = ref<Favorite[]>([])
const learningList = ref<LearningRecord[]>([])
const reviewHistory = ref<ReviewHistory[]>([])

// 学习页面筛选条件
const learningFilter = ref<'all' | 'due' | 'mastered'>('all')

// 已选择的学习记录，使用小写单词作为唯一标识。
const selectedLearningWords = ref<string[]>([])

// --------------------------------------------------
// 复习历史页面筛选状态
// --------------------------------------------------

// 单词搜索
const reviewHistorySearch = ref('')

// 熟悉程度筛选
const reviewHistoryFamiliarity = ref('all')

// 日期范围筛选
const reviewHistoryDateFrom = ref('')
const reviewHistoryDateTo = ref('')

// --------------------------------------------------
// 每日复习状态
// --------------------------------------------------

// 是否正在进行复习
const isReviewMode = ref(false)

// 本轮复习单词队列
const reviewQueue = ref<LearningRecord[]>([])

// 当前复习到第几个单词
const reviewIndex = ref(0)

// 是否已经显示当前单词的释义
const reviewAnswerVisible = ref(false)

// 当前单词的词典释义
const reviewDefinitionResults = ref<DictionarySearchResult[]>([])

// 是否正在加载释义
const isLoadingReviewDefinition = ref(false)

// 是否正在保存复习结果
const isSavingReview = ref(false)

// 熟悉程度对应的复习间隔（天）
const familiarityIntervals: Record<number, number> = {
  1: 1,
  2: 3,
  3: 7,
  4: 14,
  5: 30,
}

// 熟悉程度对应的文字说明
const familiarityLabels: Record<number, string> = {
  1: '完全不熟悉',
  2: '不太熟悉',
  3: '基本认识',
  4: '比较熟悉',
  5: '已经掌握',
}

// 页面提示
const errorMessage = ref('')
const successMessage = ref('')

// --------------------------------------------------
// 计算属性
// --------------------------------------------------

const isCurrentWordFavorite = computed(() => {
  if (!currentDefinition.value) {
    return false
  }

  return favoriteList.value.some(
    (item) =>
      item.word.toLowerCase() ===
      currentDefinition.value!.headword.toLowerCase(),
  )
})

// --------------------------------------------------
// 单词学习计算属性
// --------------------------------------------------

// 获取本地日期，格式为 YYYY-MM-DD。
// 使用本地日期，避免 UTC 时区造成日期偏差。
function getLocalDateString(date = new Date()): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')

  return `${year}-${month}-${day}`
}

// 已经到期、需要复习的单词
const dueLearningRecords = computed(() => {
  const today = getLocalDateString()

  return learningList.value.filter((item) => item.next_review_at <= today)
})

// 当前需要复习的单词数量
const dueLearningCount = computed(() => {
  return dueLearningRecords.value.length
})

// 当前复习单词
const currentReviewRecord = computed(() => {
  return reviewQueue.value[reviewIndex.value] ?? null
})

// 已经掌握的单词数量
const masteredLearningCount = computed(() => {
  return learningList.value.filter((item) => item.familiarity === 5).length
})

// 正在学习的单词数量
const studyingLearningCount = computed(() => {
  return learningList.value.filter((item) => item.familiarity < 5).length
})

// 当前释义窗口中单词的学习记录
const currentWordLearningRecord = computed(() => {
  const word = currentDefinition.value?.headword

  if (!word) {
    return null
  }

  return (
    learningList.value.find(
      (item) => item.word.toLocaleLowerCase() === word.toLocaleLowerCase(),
    ) ?? null
  )
})

// 根据筛选条件显示学习记录
const filteredLearningList = computed(() => {
  const today = getLocalDateString()

  if (learningFilter.value === 'due') {
    return learningList.value.filter((item) => item.next_review_at <= today)
  }

  if (learningFilter.value === 'mastered') {
    return learningList.value.filter((item) => item.familiarity === 5)
  }

  return learningList.value
})

// 当前筛选结果是否全部被选中。
const allVisibleLearningSelected = computed(() => {
  const records = filteredLearningList.value

  return (
    records.length > 0 &&
    records.every((item) =>
      selectedLearningWords.value.includes(item.word.toLocaleLowerCase()),
    )
  )
})

// --------------------------------------------------
// 复习历史筛选
// --------------------------------------------------

const filteredReviewHistory = computed(() => {
  const keyword = reviewHistorySearch.value.trim().toLocaleLowerCase()

  return reviewHistory.value.filter((item) => {
    // 单词搜索
    const matchesWord =
      !keyword || item.word.toLocaleLowerCase().includes(keyword)

    // 熟悉程度筛选
    const matchesFamiliarity =
      reviewHistoryFamiliarity.value === 'all' ||
      item.familiarity === Number(reviewHistoryFamiliarity.value)

    // 日期范围筛选
    const reviewDate = item.reviewed_at.slice(0, 10)

    const matchesDateFrom =
      !reviewHistoryDateFrom.value || reviewDate >= reviewHistoryDateFrom.value

    const matchesDateTo =
      !reviewHistoryDateTo.value || reviewDate <= reviewHistoryDateTo.value

    return matchesWord && matchesFamiliarity && matchesDateFrom && matchesDateTo
  })
})

// 切换学习记录筛选条件，同时清除旧选择。
function setLearningFilter(filter: 'all' | 'due' | 'mastered') {
  learningFilter.value = filter
  selectedLearningWords.value = []
}

// 全选或取消全选当前筛选结果。
function toggleSelectAllVisibleLearning() {
  if (allVisibleLearningSelected.value) {
    selectedLearningWords.value = []
    return
  }

  selectedLearningWords.value = filteredLearningList.value.map((item) =>
    item.word.toLocaleLowerCase(),
  )
}

// 当前筛选条件是否已经启用
const hasReviewHistoryFilters = computed(() => {
  return (
    reviewHistorySearch.value.trim() !== '' ||
    reviewHistoryFamiliarity.value !== 'all' ||
    reviewHistoryDateFrom.value !== '' ||
    reviewHistoryDateTo.value !== ''
  )
})

// --------------------------------------------------
// 学习数据统计
// --------------------------------------------------

// 今日复习次数
const todayReviewCount = computed(() => {
  const today = getLocalDateString()

  return reviewHistory.value.filter(
    (item) => item.reviewed_at.slice(0, 10) === today,
  ).length
})

// 累计复习次数
const totalReviewCount = computed(() => {
  return reviewHistory.value.length
})

// 历史上复习过的不同单词数量
const reviewedWordCount = computed(() => {
  const words = new Set(
    reviewHistory.value.map((item) => item.word.toLocaleLowerCase()),
  )

  return words.size
})

// 最近 7 天每日复习次数
const reviewDailyStats = computed(() => {
  const counts = new Map<string, number>()

  for (const item of reviewHistory.value) {
    const date = item.reviewed_at.slice(0, 10)
    counts.set(date, (counts.get(date) ?? 0) + 1)
  }

  const result: {
    date: string
    label: string
    count: number
  }[] = []

  for (let i = 6; i >= 0; i--) {
    const date = new Date()
    date.setHours(0, 0, 0, 0)
    date.setDate(date.getDate() - i)

    const dateString = getLocalDateString(date)

    result.push({
      date: dateString,
      label: `${date.getMonth() + 1}/${date.getDate()}`,
      count: counts.get(dateString) ?? 0,
    })
  }

  return result
})

// 最近 7 天最高单日复习次数
const maxDailyReviewCount = computed(() => {
  return Math.max(1, ...reviewDailyStats.value.map((item) => item.count))
})

// 连续学习天数
const reviewStreak = computed(() => {
  const activeDates = new Set(
    reviewHistory.value.map((item) => item.reviewed_at.slice(0, 10)),
  )

  const cursor = new Date()
  cursor.setHours(0, 0, 0, 0)

  // 如果今天尚未复习，则从昨天开始计算连续天数。
  if (!activeDates.has(getLocalDateString(cursor))) {
    cursor.setDate(cursor.getDate() - 1)
  }

  let streak = 0

  while (activeDates.has(getLocalDateString(cursor))) {
    streak++
    cursor.setDate(cursor.getDate() - 1)
  }

  return streak
})

// 当前学习单词的熟悉程度分布
const familiarityDistribution = computed(() => {
  return [1, 2, 3, 4, 5].map((level) => ({
    level,
    label: familiarityLabels[level],
    count: learningList.value.filter((item) => item.familiarity === level)
      .length,
  }))
})

// 已掌握单词占比
const masteryPercentage = computed(() => {
  if (learningList.value.length === 0) {
    return 0
  }

  return Math.round(
    (masteredLearningCount.value / learningList.value.length) * 100,
  )
})

// 清理词典释义中的 HTML
function sanitizeDefinition(definition: string) {
  return DOMPurify.sanitize(definition, {
    USE_PROFILES: { html: true },
  })
}

// --------------------------------------------------
// 提示信息
// --------------------------------------------------

function showError(message: string) {
  errorMessage.value = message
  successMessage.value = ''
}

function showSuccess(message: string) {
  successMessage.value = message
  errorMessage.value = ''
}

function clearMessages() {
  errorMessage.value = ''
  successMessage.value = ''
}

// --------------------------------------------------
// 加载搜索历史
// --------------------------------------------------

async function loadHistory() {
  try {
    historyList.value = await invoke<SearchHistory[]>('get_search_history')
  } catch (error) {
    console.error('加载搜索历史失败：', error)
    showError(`加载搜索历史失败：${String(error)}`)
  }
}

// 刷新词典列表
async function refreshDictionaries() {
  dictionaries.value = await invoke<DictionaryInfo[]>('get_dictionaries')

  if (
    activeDictionaryPath.value &&
    !dictionaries.value.some(
      (item) => item.path === activeDictionaryPath.value && item.enabled,
    )
  ) {
    activeDictionaryPath.value = ''
    dictionaryInfo.value = null
  }
}

// 添加词典文件
async function addDictionary() {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: 'MDX Dictionary',
          extensions: ['mdx'],
        },
      ],
    })

    if (!selected || Array.isArray(selected)) {
      return
    }

    const info = await invoke<DictionaryInfo>('open_dictionary', {
      path: selected,
    })

    await refreshDictionaries()

    dictionaryInfo.value = info
    activeDictionaryPath.value = info.path

    await invoke('save_dictionary_path', {
      path: info.path,
    })

    saveDictionaryLibrary()

    saveDisabledDictionaries()

    currentPage.value = 'dictionaries'
  } catch (error) {
    console.error('添加词典失败：', error)
    alert(`添加词典失败：${error}`)
  }
}

// 切换当前使用的词典。
async function activateDictionary(path: string) {
  try {
    const info = await invoke<DictionaryInfo>('switch_dictionary', { path })

    if (!info.enabled) {
      showError('该词典已经停用，无法切换。')
      return
    }

    dictionaryInfo.value = info
    activeDictionaryPath.value = info.path

    await invoke('save_dictionary_path', {
      path: info.path,
    })

    currentPage.value = 'search'
  } catch (error) {
    console.error('切换词典失败：', error)
    showError(`切换词典失败：${String(error)}`)
  }
}

// 启用或停用词典。
async function toggleDictionaryEnabled(path: string) {
  const target = dictionaries.value.find((item) => item.path === path)

  if (!target) {
    return
  }

  const nextEnabled = !target.enabled

  try {
    // 调用 Rust 后端改变词典状态。
    const activeInfo = await invoke<DictionaryInfo | null>(
      'set_dictionary_enabled',
      {
        path,
        enabled: nextEnabled,
      },
    )

    // 更新词典列表。
    await refreshDictionaries()

    // 保存启用状态。
    saveDisabledDictionaries()

    // 同步当前活动词典。
    if (activeInfo) {
      dictionaryInfo.value = activeInfo
      activeDictionaryPath.value = activeInfo.path

      // 同步上次使用的词典路径。
      await invoke('save_dictionary_path', {
        path: activeInfo.path,
      })
    } else {
      dictionaryInfo.value = null
      activeDictionaryPath.value = ''

      await invoke('save_dictionary_path', {
        path: '',
      })
    }

    // 如果当前词典被停用，自动切换搜索范围。
    if (!dictionaryInfo.value && searchScope.value === 'current') {
      searchScope.value = 'all'
    }

    const message = nextEnabled
      ? `已启用词典：${target.name}`
      : `已停用词典：${target.name}`

    showSuccess(message)
  } catch (error) {
    console.error('修改词典启用状态失败：', error)
    showError(`修改词典启用状态失败：${String(error)}`)
  }
}

// 移除词典
async function deleteDictionary(path: string) {
  const target = dictionaries.value.find((item) => item.path === path)

  if (!target) {
    return
  }

  const confirmed = window.confirm(
    `确定从词典库中移除「${target.name}」吗？\n\n不会删除硬盘上的 MDX 文件。`,
  )

  if (!confirmed) {
    return
  }

  try {
    const wasActive = activeDictionaryPath.value === path

    await invoke('remove_dictionary', { path })

    await refreshDictionaries()

    if (wasActive) {
      const nextDictionary = enabledDictionaries.value[0]

      if (nextDictionary) {
        await activateDictionary(nextDictionary.path)
      } else {
        activeDictionaryPath.value = ''
        dictionaryInfo.value = null

        await invoke('save_dictionary_path', {
          path: '',
        })
      }
    }

    // 删除词典后同步两个持久化列表。
    saveDictionaryLibrary()
    saveDisabledDictionaries()
  } catch (error) {
    console.error('移除词典失败：', error)
    alert(`移除词典失败：${error}`)
  }
}

// 启动恢复

async function restoreDictionaryLibrary() {
  const saved = localStorage.getItem(DICTIONARY_LIBRARY_KEY)

  if (!saved) {
    await restoreLastDictionary()

    await refreshDictionaries()

    if (dictionaryInfo.value) {
      activeDictionaryPath.value = dictionaryInfo.value.path
    }

    return
  }

  // 读取之前停用的词典路径。
  let disabledPaths: string[] = []

  try {
    const savedDisabled = localStorage.getItem(DISABLED_DICTIONARIES_KEY)

    disabledPaths = savedDisabled ? JSON.parse(savedDisabled) : []
  } catch {
    disabledPaths = []
  }

  let paths: string[]

  try {
    paths = JSON.parse(saved)
  } catch {
    paths = []
  }

  const validPaths: string[] = []

  for (const path of paths) {
    try {
      await invoke<DictionaryInfo>('open_dictionary', { path })

      validPaths.push(path)
    } catch (error) {
      console.warn(`无法恢复词典：${path}`, error)
    }
  }

  await refreshDictionaries()

  // 恢复之前的停用状态。
  for (const path of disabledPaths) {
    const dictionary = dictionaries.value.find((item) => item.path === path)

    if (!dictionary) {
      continue
    }

    try {
      await invoke('set_dictionary_enabled', {
        path,
        enabled: false,
      })
    } catch (error) {
      console.warn(`恢复词典停用状态失败：${path}`, error)
    }
  }

  await refreshDictionaries()

  // 优先恢复上一次使用的、仍然启用的词典。
  let lastPath = ''

  try {
    lastPath = await invoke<string>('get_dictionary_path')
  } catch {
    // 没有保存过上次使用的词典
  }

  const target =
    enabledDictionaries.value.find((item) => item.path === lastPath) ??
    enabledDictionaries.value[0]

  if (target) {
    await activateDictionary(target.path)
  } else {
    dictionaryInfo.value = null
    activeDictionaryPath.value = ''
  }

  localStorage.setItem(DICTIONARY_LIBRARY_KEY, JSON.stringify(validPaths))

  saveDisabledDictionaries()
}

// --------------------------------------------------
// 加载收藏
// --------------------------------------------------

async function loadFavorites() {
  try {
    favoriteList.value = await invoke<Favorite[]>('get_favorites')
  } catch (error) {
    console.error('加载收藏失败：', error)
    showError(`加载收藏失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 单词学习记录
// --------------------------------------------------

// 加载全部学习记录
async function loadLearningRecords() {
  try {
    learningList.value = await invoke<LearningRecord[]>('get_learning_records')
  } catch (error) {
    console.error('加载学习记录失败：', error)
    showError(`加载学习记录失败：${String(error)}`)
  }
}

// 加载复习历史
async function loadReviewHistory() {
  try {
    reviewHistory.value = await invoke<ReviewHistory[]>('get_review_history')
  } catch (error) {
    console.error('加载复习历史失败：', error)
    showError(`加载复习历史失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 复习历史管理
// --------------------------------------------------

// 重置复习历史筛选条件
function resetReviewHistoryFilters() {
  reviewHistorySearch.value = ''
  reviewHistoryFamiliarity.value = 'all'
  reviewHistoryDateFrom.value = ''
  reviewHistoryDateTo.value = ''
}

// CSV 字段转义
function escapeCsvField(value: string | number): string {
  const text = String(value)
  return `"${text.replace(/"/g, '""')}"`
}

// 导出当前筛选后的复习历史
async function exportReviewHistoryCsv() {
  const records = filteredReviewHistory.value

  if (records.length === 0) {
    showError('当前没有可以导出的复习记录。')
    return
  }

  const headers = ['ID', '单词', '熟悉程度', '熟悉程度说明', '复习时间']

  const rows = records.map((item) => [
    item.id,
    item.word,
    item.familiarity,
    familiarityLabels[item.familiarity],
    item.reviewed_at,
  ])

  const csvContent = [
    headers.map(escapeCsvField).join(','),
    ...rows.map((row) => row.map(escapeCsvField).join(',')),
  ].join('\r\n')

  try {
    // 弹出 Windows 原生文件保存对话框。
    const filePath = await save({
      title: '导出复习历史',
      defaultPath: `word-review-history-${getLocalDateString()}.csv`,
      filters: [
        {
          name: 'CSV 文件',
          extensions: ['csv'],
        },
      ],
    })

    // 用户取消保存。
    if (!filePath) {
      return
    }

    // 调用 Rust 后端，将 CSV 内容写入指定文件。
    await invoke('write_export_file', {
      path: filePath,
      contents: '\uFEFF' + csvContent,
    })

    showSuccess(
      `成功导出 ${records.length} 条复习历史记录。\n保存位置：${filePath}`,
    )
  } catch (error) {
    console.error('导出复习历史失败：', error)
    showError(`导出复习历史失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 应用数据备份与恢复
// --------------------------------------------------

// 导出全部应用数据
async function exportAppBackup() {
  try {
    // 从 Rust 获取完整 JSON 备份。
    const contents = await invoke<string>('export_app_backup')

    // 弹出文件保存窗口。
    const filePath = await save({
      title: '备份小猪词典数据',
      defaultPath: `小猪词典备份-${getLocalDateString()}.json`,
      filters: [
        {
          name: 'JSON 备份文件',
          extensions: ['json'],
        },
      ],
    })

    if (!filePath) {
      return
    }

    // 写入文件。
    await invoke('write_export_file', {
      path: filePath,
      contents,
    })

    showSuccess(`数据备份成功。\n保存位置：${filePath}`)
  } catch (error) {
    console.error('数据备份失败：', error)
    showError(`数据备份失败：${String(error)}`)
  }
}

// 从备份文件恢复应用数据
async function restoreAppBackup() {
  try {
    // 选择 JSON 备份文件。
    const selected = await open({
      title: '选择 小猪词典 数据备份',
      multiple: false,
      filters: [
        {
          name: 'JSON 备份文件',
          extensions: ['json'],
        },
      ],
    })

    if (typeof selected !== 'string') {
      return
    }

    // 读取备份文件信息。
    const preview = await invoke<BackupPreview>('inspect_app_backup', {
      path: selected,
    })

    // 生成备份内容预览。
    const previewMessage = [
      '即将恢复以下备份数据：',
      '',
      `备份生成时间：${preview.exported_at}`,
      '',
      `搜索历史：${preview.search_history_count} 条`,
      `收藏：${preview.favorites_count} 条`,
      `学习记录：${preview.learning_records_count} 条`,
      `复习历史：${preview.review_history_count} 条`,
      '',
      '注意：恢复操作会覆盖当前四类数据。',
      '建议先导出当前数据作为备份。',
      '',
      '确定继续恢复吗？',
    ].join('\n')

    const confirmed = window.confirm(previewMessage)

    if (!confirmed) {
      return
    }

    // 调用 Rust 后端恢复数据库。
    await invoke('restore_app_backup', {
      path: selected,
    })

    // 刷新前端数据。
    await Promise.all([
      loadHistory(),
      loadFavorites(),
      loadLearningRecords(),
      loadReviewHistory(),
    ])

    // 清除旧的复习历史筛选条件。
    resetReviewHistoryFilters()

    // 退出正在进行的复习，避免继续使用旧队列。
    isReviewMode.value = false
    reviewQueue.value = []
    reviewIndex.value = 0
    reviewAnswerVisible.value = false
    reviewDefinitionResults.value = []

    showSuccess(
      `数据恢复成功！\n` +
        `搜索历史：${preview.search_history_count} 条\n` +
        `收藏：${preview.favorites_count} 条\n` +
        `学习记录：${preview.learning_records_count} 条\n` +
        `复习历史：${preview.review_history_count} 条`,
    )
  } catch (error) {
    console.error('数据恢复失败：', error)
    showError(`数据恢复失败：${String(error)}`)
  }
}

// 保存或更新学习记录
async function saveLearningRecord(familiarity: number) {
  if (!currentDefinition.value) {
    return
  }

  if (!(familiarity in familiarityIntervals)) {
    return
  }

  const word = currentDefinition.value.headword
  const interval = familiarityIntervals[familiarity]

  // 根据当前本地日期计算下次复习日期
  const nextReviewDate = new Date()
  nextReviewDate.setDate(nextReviewDate.getDate() + interval)

  const nextReviewAt = getLocalDateString(nextReviewDate)

  try {
    await invoke('save_learning_record', {
      word,
      familiarity,
      nextReviewAt,
    })

    await Promise.all([loadLearningRecords(), loadReviewHistory()])

    showSuccess(`已更新「${word}」的学习记录，下次复习日期：${nextReviewAt}`)
  } catch (error) {
    console.error('保存学习记录失败：', error)
    showError(`保存学习记录失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 每日单词复习
// --------------------------------------------------

// 开始一轮复习
function startReview() {
  if (enabledDictionaries.value.length === 0) {
    showError('请先启用至少一本 MDX 词典，再开始复习。')
    return
  }

  const records = [...dueLearningRecords.value]

  if (records.length === 0) {
    showSuccess('今天没有需要复习的单词。')
    return
  }

  reviewQueue.value = records
  reviewIndex.value = 0
  reviewAnswerVisible.value = false
  reviewDefinitionResults.value = []

  isReviewMode.value = true
  clearMessages()
}

// 显示当前单词的释义
async function showReviewAnswer() {
  const record = currentReviewRecord.value

  if (!record || isLoadingReviewDefinition.value) {
    return
  }

  isLoadingReviewDefinition.value = true
  reviewDefinitionResults.value = []

  try {
    const results = await invoke<DictionarySearchResult[]>(
      'search_all_dictionaries',
      {
        word: record.word,
      },
    )

    reviewDefinitionResults.value = results
    reviewAnswerVisible.value = true
  } catch (error) {
    console.error('加载复习释义失败：', error)
    showError(`加载「${record.word}」的释义失败：${String(error)}`)
  } finally {
    isLoadingReviewDefinition.value = false
  }
}

// 提交当前单词的复习结果
async function rateReview(familiarity: number) {
  const record = currentReviewRecord.value

  if (!record || !reviewAnswerVisible.value || isSavingReview.value) {
    return
  }

  if (!(familiarity in familiarityIntervals)) {
    return
  }

  isSavingReview.value = true

  try {
    const interval = familiarityIntervals[familiarity]

    // 根据当前本地日期计算下次复习日期
    const nextReviewDate = new Date()
    nextReviewDate.setDate(nextReviewDate.getDate() + interval)

    const nextReviewAt = getLocalDateString(nextReviewDate)

    await invoke('save_learning_record', {
      word: record.word,
      familiarity,
      nextReviewAt,
    })

    // 更新本地学习记录和复习历史
    await Promise.all([loadLearningRecords(), loadReviewHistory()])

    // 进入下一个单词
    reviewIndex.value++
    reviewAnswerVisible.value = false
    reviewDefinitionResults.value = []

    // 本轮复习结束
    if (reviewIndex.value >= reviewQueue.value.length) {
      const completedCount = reviewQueue.value.length

      isReviewMode.value = false
      reviewQueue.value = []
      reviewIndex.value = 0

      showSuccess(`本轮复习完成！共复习 ${completedCount} 个单词。`)
    }
  } catch (error) {
    console.error('保存复习结果失败：', error)
    showError(`保存「${record.word}」的复习结果失败：${String(error)}`)
  } finally {
    isSavingReview.value = false
  }
}

// 退出当前复习
function exitReview() {
  const confirmed = window.confirm(
    '确定退出本轮复习吗？已经完成的复习结果会保留。',
  )

  if (!confirmed) {
    return
  }

  isReviewMode.value = false
  reviewQueue.value = []
  reviewIndex.value = 0
  reviewAnswerVisible.value = false
  reviewDefinitionResults.value = []
}

// 删除学习记录
async function removeLearningRecord(word: string) {
  const confirmed = window.confirm(`确定删除「${word}」的学习记录吗？`)

  if (!confirmed) {
    return
  }

  try {
    await invoke('remove_learning_record', {
      word,
    })

    await loadLearningRecords()

    showSuccess(`已删除「${word}」的学习记录。`)
  } catch (error) {
    console.error('删除学习记录失败：', error)
    showError(`删除学习记录失败：${String(error)}`)
  }
}

// 批量删除已选择的学习记录。
async function removeSelectedLearningRecords() {
  if (selectedLearningWords.value.length === 0) {
    return
  }

  const selectedSet = new Set(selectedLearningWords.value)

  // 从完整学习记录中获取实际要删除的单词。
  const words = learningList.value
    .filter((item) => selectedSet.has(item.word.toLocaleLowerCase()))
    .map((item) => item.word)

  if (words.length === 0) {
    selectedLearningWords.value = []
    return
  }

  const confirmed = window.confirm(
    `确定删除选中的 ${words.length} 条学习记录吗？\n\n` +
      '这只会删除当前学习状态，不会删除这些单词的历史复习记录。',
  )

  if (!confirmed) {
    return
  }

  try {
    const deletedCount = await invoke<number>('remove_learning_records', {
      words,
    })

    selectedLearningWords.value = []

    await loadLearningRecords()

    // 删除记录后，当前复习队列可能包含已删除单词。
    isReviewMode.value = false
    reviewQueue.value = []
    reviewIndex.value = 0
    reviewAnswerVisible.value = false
    reviewDefinitionResults.value = []

    showSuccess(`成功删除 ${deletedCount} 条学习记录，复习历史已保留。`)
  } catch (error) {
    console.error('批量删除学习记录失败：', error)
    showError(`批量删除学习记录失败：${String(error)}`)
  }
}

// 清空全部复习历史。
async function clearReviewHistory() {
  if (reviewHistory.value.length === 0) {
    return
  }

  const confirmed = window.confirm(
    `确定清空全部 ${reviewHistory.value.length} 条复习历史吗？\n\n` +
      '此操作不可撤销，但不会删除当前学习记录。',
  )

  if (!confirmed) {
    return
  }

  try {
    const deletedCount = await invoke<number>('clear_review_history')

    await loadReviewHistory()

    showSuccess(`已清空 ${deletedCount} 条复习历史。`)
  } catch (error) {
    console.error('清空复习历史失败：', error)
    showError(`清空复习历史失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 恢复上次打开的词典
// --------------------------------------------------

async function restoreLastDictionary() {
  try {
    const path = await invoke<string | null>('get_dictionary_path')

    if (!path) {
      return
    }

    dictionaryPath.value = path

    const info = await invoke<DictionaryInfo>('open_dictionary', { path })

    dictionaryInfo.value = info

    console.log('已恢复上次打开的词典：', info.name)
  } catch (error) {
    console.warn('恢复上次词典失败：', error)

    dictionaryInfo.value = null

    showError('无法恢复上次打开的词典，请重新选择 MDX 文件。')
  }
}

// --------------------------------------------------
// 实时搜索建议
// --------------------------------------------------

async function fetchSuggestions(query: string) {
  const requestId = ++suggestionRequestId

  if (!query.trim()) {
    suggestions.value = []
    isSuggestionOpen.value = false
    return
  }

  try {
    let dictionaryWords: string[] = []

    // ----------------------------------------------
    // 根据搜索范围获取词典前缀建议
    // ----------------------------------------------

    if (searchScope.value === 'all') {
      // 搜索全部已加载词典。
      if (enabledDictionaries.value.length > 0) {
        dictionaryWords = await invoke<string[]>(
          'suggest_words_all_dictionaries',
          {
            prefix: query,
            limit: 20,
          },
        )
      }
    } else {
      // 只搜索当前活动词典。
      if (dictionaryInfo.value) {
        dictionaryWords = await invoke<string[]>('suggest_words', {
          prefix: query,
          limit: 20,
        })
      }
    }

    // 防止较早发出的请求覆盖较新的搜索结果。
    if (requestId !== suggestionRequestId) {
      return
    }

    const normalizedQuery = query.toLocaleLowerCase()

    const result: SearchSuggestion[] = []
    const seen = new Set<string>()

    function addSuggestion(word: string, source: SearchSuggestion['source']) {
      const normalizedWord = word.toLocaleLowerCase()

      if (
        !normalizedWord.startsWith(normalizedQuery) ||
        seen.has(normalizedWord)
      ) {
        return
      }

      seen.add(normalizedWord)

      result.push({
        word,
        source,
      })
    }

    // 优先显示收藏中的匹配单词
    for (const item of favoriteList.value) {
      addSuggestion(item.word, '收藏')
    }

    // 其次显示历史记录中的匹配单词
    for (const item of historyList.value) {
      addSuggestion(item.word, '历史')
    }

    // 最后显示词典中的前缀匹配结果
    for (const word of dictionaryWords) {
      addSuggestion(word, '词典')
    }

    suggestions.value = result.slice(0, 8)
    isSuggestionOpen.value = suggestions.value.length > 0
    selectedSuggestionIndex.value = -1
  } catch (error) {
    console.error('获取搜索建议失败：', error)

    suggestions.value = []
    isSuggestionOpen.value = false
  }
}

// --------------------------------------------------
// 切换搜索范围
// --------------------------------------------------

function changeSearchScope(scope: 'all' | 'current') {
  if (scope === 'current' && !dictionaryInfo.value) {
    return
  }

  searchScope.value = scope

  // 如果搜索框已有内容，立即刷新搜索建议。
  const query = searchText.value.trim()

  if (query) {
    fetchSuggestions(query)
  }
}

// --------------------------------------------------
// 搜索框输入事件
// --------------------------------------------------

function handleSearchInput() {
  const query = searchText.value.trim()

  // 取消尚未执行的请求
  if (suggestionTimer) {
    clearTimeout(suggestionTimer)
  }

  // 立即使之前尚未完成的请求失效
  suggestionRequestId++

  selectedSuggestionIndex.value = -1

  if (!query) {
    suggestionRequestId++
    suggestions.value = []
    isSuggestionOpen.value = false
    return
  }

  // 180ms 防抖，避免每输入一个字符就立即请求
  suggestionTimer = setTimeout(() => {
    fetchSuggestions(query)
  }, 180)
}

// --------------------------------------------------
// 选择搜索建议
// --------------------------------------------------

function selectSuggestion(word: string) {
  if (suggestionTimer) {
    clearTimeout(suggestionTimer)
  }

  suggestionRequestId++

  searchText.value = word
  isSuggestionOpen.value = false
  suggestions.value = []
  selectedSuggestionIndex.value = -1

  searchWord(word)
}

// --------------------------------------------------
// 搜索框键盘操作
// --------------------------------------------------

function handleSearchKeydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown') {
    if (!suggestions.value.length) {
      return
    }

    event.preventDefault()

    selectedSuggestionIndex.value =
      selectedSuggestionIndex.value >= suggestions.value.length - 1
        ? 0
        : selectedSuggestionIndex.value + 1

    return
  }

  if (event.key === 'ArrowUp') {
    if (!suggestions.value.length) {
      return
    }

    event.preventDefault()

    selectedSuggestionIndex.value =
      selectedSuggestionIndex.value <= 0
        ? suggestions.value.length - 1
        : selectedSuggestionIndex.value - 1

    return
  }

  if (event.key === 'Enter') {
    if (isSuggestionOpen.value && selectedSuggestionIndex.value >= 0) {
      event.preventDefault()

      const selected = suggestions.value[selectedSuggestionIndex.value]

      if (selected) {
        selectSuggestion(selected.word)
      }
    }

    return
  }

  if (event.key === 'Escape') {
    isSuggestionOpen.value = false
    selectedSuggestionIndex.value = -1
  }
}

// --------------------------------------------------
// 搜索单词
// --------------------------------------------------

async function searchWord(word?: string) {
  isSuggestionOpen.value = false
  suggestions.value = []
  selectedSuggestionIndex.value = -1

  if (suggestionTimer) {
    clearTimeout(suggestionTimer)
  }

  suggestionRequestId++

  const query = (word ?? searchText.value).trim()

  if (!query) {
    showError('请输入要查询的英文单词。')
    return
  }

  if (enabledDictionaries.value.length === 0) {
    showError(
      '当前没有可用的 MDX 词典，请前往“词典管理”页面添加并启用本地词典。',
    )
    return
  }

  clearMessages()
  isSearching.value = true
  searchText.value = query

  try {
    let results: DictionarySearchResult[] = []

    // ----------------------------------------------
    // 全部词典搜索
    // ----------------------------------------------

    if (searchScope.value === 'all') {
      results = await invoke<DictionarySearchResult[]>(
        'search_all_dictionaries',
        {
          word: query,
        },
      )
    }

    // ----------------------------------------------
    // 当前词典搜索
    // ----------------------------------------------
    else {
      const result = await invoke<WordResult | null>('search_word', {
        word: query,
      })

      if (result && dictionaryInfo.value) {
        results = [
          {
            headword: result.headword,
            definition: result.definition,
            dictionary_name: dictionaryInfo.value.name,
            dictionary_path: dictionaryInfo.value.path,
            match_type: 'exact',
            distance: null,
          },
        ]
      }
    }

    // 保存搜索历史
    await invoke('add_search_history', {
      word: query,
    })

    await loadHistory()

    // 没有搜索结果
    if (results.length === 0) {
      searchResults.value = []
      currentDefinition.value = null
      currentDictionaryPath.value = ''
      selectedDictionaryByHeadword.value = {}

      showError(`没有找到单词：${query}`)
      return
    }

    // 保存全部词典搜索结果
    searchResults.value = results

    // 每次新搜索重置筛选条件。
    selectedDictionaryFilter.value = 'all'
    selectedMatchType.value = 'all'

    // 清空上一次搜索的词典选择。
    selectedDictionaryByHeadword.value = {}

    // 按照排序后的结果，为每个词头设置默认词典。
    for (const item of sortedSearchResults.value) {
      const key = item.headword.toLocaleLowerCase()

      if (!selectedDictionaryByHeadword.value[key]) {
        selectedDictionaryByHeadword.value[key] = item.dictionary_path
      }
    }

    // 默认选中排序后的第一个结果。
    const firstResult = sortedSearchResults.value[0]

    currentDefinition.value = {
      headword: firstResult.headword,
      definition: firstResult.definition,
    }

    currentDictionaryPath.value = firstResult.dictionary_path

    // 显示释义窗口
    isDefinitionOpen.value = true
  } catch (error) {
    console.error('搜索单词失败：', error)

    searchResults.value = []
    currentDefinition.value = null
    currentDictionaryPath.value = ''
    selectedDictionaryByHeadword.value = {}

    showError(`搜索失败：${String(error)}`)
  } finally {
    isSearching.value = false
  }
}

// --------------------------------------------------
// 关闭释义窗口
// --------------------------------------------------

function closeDefinition() {
  isDefinitionOpen.value = false
}

// --------------------------------------------------
// 添加或取消收藏
// --------------------------------------------------

async function toggleFavorite() {
  if (!currentDefinition.value) {
    return
  }

  const word = currentDefinition.value.headword

  try {
    if (isCurrentWordFavorite.value) {
      await invoke('remove_favorite', { word })
      showSuccess(`已取消收藏：${word}`)
    } else {
      await invoke('add_favorite', { word })
      showSuccess(`已收藏：${word}`)
    }

    await loadFavorites()
  } catch (error) {
    console.error('修改收藏失败：', error)
    showError(`修改收藏失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 从历史记录或收藏中查询单词
// --------------------------------------------------

async function searchFromList(word: string) {
  currentPage.value = 'search'
  searchText.value = word

  await searchWord(word)
}

// --------------------------------------------------
// 清空搜索历史
// --------------------------------------------------

async function clearHistory() {
  if (historyList.value.length === 0) {
    return
  }

  const confirmed = window.confirm('确定要清空全部搜索历史吗？此操作不可撤销。')

  if (!confirmed) {
    return
  }

  try {
    await invoke('clear_search_history')
    historyList.value = []
    showSuccess('搜索历史已清空。')
  } catch (error) {
    console.error('清空搜索历史失败：', error)
    showError(`清空搜索历史失败：${String(error)}`)
  }
}

// --------------------------------------------------
// 页面切换
// --------------------------------------------------

function switchPage(page: PageName) {
  currentPage.value = page
  clearMessages()
}

// --------------------------------------------------
// 全局键盘快捷键
// --------------------------------------------------

function handleGlobalKeydown(event: KeyboardEvent) {
  // Ctrl + K 或 Cmd + K：快速聚焦搜索框
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault()

    currentPage.value = 'search'

    nextTick(() => {
      searchInput.value?.focus()
    })
  }

  // Esc：关闭释义窗口和搜索建议
  if (event.key === 'Escape') {
    if (isDefinitionOpen.value) {
      closeDefinition()
    }

    isSuggestionOpen.value = false
    selectedSuggestionIndex.value = -1
  }
}

// --------------------------------------------------
// 日期格式化
// --------------------------------------------------

function formatDate(dateString: string) {
  if (!dateString) {
    return ''
  }

  return dateString.replace('T', ' ')
}

// --------------------------------------------------
// 应用启动
// --------------------------------------------------

onMounted(async () => {
  window.addEventListener('keydown', handleGlobalKeydown)

  // 根据窗口宽度自动调整左侧导航栏。
  syncSidebarLayout()
  window.addEventListener('resize', syncSidebarLayout)

  // 加载搜索历史
  await loadHistory()

  // 加载收藏
  await loadFavorites()

  // 加载单词学习记录
  await loadLearningRecords()

  // 加载复习历史
  await loadReviewHistory()

  // 恢复上次打开的词典
  await restoreDictionaryLibrary()
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
  window.removeEventListener('resize', syncSidebarLayout)

  if (suggestionTimer) {
    clearTimeout(suggestionTimer)
  }
})
</script>

<template>
  <div class="app-container">
    <!-- 左侧导航栏 -->
    <aside class="sidebar" :class="{ 'sidebar-collapsed': isSidebarCollapsed }">
      <!-- 可滚动的导航区域 -->
      <div class="sidebar-navigation">
        <div class="sidebar-brand">
          <div class="brand-icon">
            <BookOpen :size="23" />
          </div>

          <span class="brand-name">小猪词典</span>
        </div>

        <div class="sidebar-section">
          <button
            class="nav-item"
            :class="{ active: currentPage === 'search' }"
            :title="isSidebarCollapsed ? '单词查询' : undefined"
            :aria-label="'单词查询'"
            @click="switchPage('search')"
          >
            <Search :size="18" />
            <span>单词查询</span>
          </button>

          <button
            class="nav-item"
            :class="{ active: currentPage === 'history' }"
            :title="isSidebarCollapsed ? '搜索历史' : undefined"
            :aria-label="'搜索历史'"
            @click="switchPage('history')"
          >
            <Clock3 :size="18" />
            <span>搜索历史</span>
            <span v-if="historyList.length" class="nav-count">
              {{ historyList.length }}
            </span>
          </button>

          <button
            class="nav-item"
            :class="{ active: currentPage === 'dictionaries' }"
            :title="isSidebarCollapsed ? '词典管理' : undefined"
            :aria-label="'词典管理'"
            @click="currentPage = 'dictionaries'"
          >
            <BookOpen :size="18" />
            <span>词典管理</span>
          </button>

          <button
            class="nav-item"
            :class="{ active: currentPage === 'favorites' }"
            :title="isSidebarCollapsed ? '我的收藏' : undefined"
            :aria-label="'我的收藏'"
            @click="switchPage('favorites')"
          >
            <Bookmark :size="18" />
            <span>我的收藏</span>
            <span v-if="favoriteList.length" class="nav-count">
              {{ favoriteList.length }}
            </span>
          </button>

          <button
            class="nav-item"
            :class="{ active: currentPage === 'learning' }"
            :title="isSidebarCollapsed ? '单词学习' : undefined"
            :aria-label="'单词学习'"
            @click="switchPage('learning')"
          >
            <GraduationCap :size="18" />
            <span>单词学习</span>

            <span v-if="dueLearningCount" class="nav-count">
              {{ dueLearningCount }}
            </span>
          </button>

          <button
            class="nav-item"
            :class="{ active: currentPage === 'review-history' }"
            :title="isSidebarCollapsed ? '复习历史' : undefined"
            :aria-label="'复习历史'"
            @click="switchPage('review-history')"
          >
            <CalendarCheck :size="18" />
            <span>复习历史</span>

            <span v-if="reviewHistory.length" class="nav-count">
              {{ reviewHistory.length }}
            </span>
          </button>
        </div>
      </div>

      <!-- 固定在左下方的信息 -->
      <div class="sidebar-bottom">
        <!-- 固定在底部信息上方的收缩按钮 -->
        <div class="sidebar-toggle-container">
          <button
            class="sidebar-toggle"
            type="button"
            :title="isSidebarCollapsed ? '展开导航栏' : '收缩导航栏'"
            :aria-label="isSidebarCollapsed ? '展开导航栏' : '收缩导航栏'"
            @click="toggleSidebar"
          >
            <PanelLeftOpen v-if="isSidebarCollapsed" :size="17" />
            <PanelLeftClose v-else :size="17" />

            <span v-if="!isSidebarCollapsed">收缩导航栏</span>
          </button>
        </div>
      </div>
    </aside>

    <!-- 主内容区域 -->
    <main class="main-content">
      <!-- 顶部栏 -->
      <header class="topbar" data-tauri-drag-region>
  <!-- 左侧页面路径 -->
  <div class="breadcrumb" data-tauri-drag-region>
    <span>小猪词典</span>
    <ChevronRight :size="15" />
    <strong>
      {{
        currentPage === 'search'
          ? '单词查询'
          : currentPage === 'history'
            ? '搜索历史'
            : currentPage === 'favorites'
              ? '我的收藏'
              : currentPage === 'learning'
                ? '单词学习'
                : currentPage === 'review-history'
                  ? '复习历史'
                  : currentPage === 'dictionaries'
                    ? '词典管理'
                    : '设置'
      }}
    </strong>
  </div>

  <!-- 右侧窗口控制按钮 -->
  <div class="window-controls">

    <!-- 设置 -->
    <button
      class="window-control-button settings-control"
      title="设置"
      aria-label="设置"
      @click="openSettings"
    >
      <Settings :size="17" />
    </button>

    <!-- 三个竖排圆点 -->
    <div
      class="window-control-separator"
      aria-hidden="true"
    >
      <MoreVertical :size="19" />
    </div>

    <!-- 最小化 -->
    <button
      class="window-control-button"
      title="最小化"
      aria-label="最小化"
      @click="minimizeWindow"
    >
      <Minus :size="17" />
    </button>

    <!-- 最大化 / 恢复 -->
    <button
      class="window-control-button"
      title="最大化或恢复"
      aria-label="最大化或恢复"
      @click="toggleWindowMaximize"
    >
      <Maximize2 :size="15" />
    </button>

    <!-- 关闭 -->
    <button
      class="window-control-button close-control"
      title="关闭"
      aria-label="关闭"
      @click="closeWindow"
    >
      <X :size="17" />
    </button>

  </div>
</header>

      <!-- 消息提示 -->
      <div v-if="errorMessage" class="message message-error">
        <AlertCircle :size="17" />
        <span>{{ errorMessage }}</span>
        <button @click="clearMessages">
          <X :size="16" />
        </button>
      </div>

      <div v-if="successMessage" class="message message-success">
        <Check :size="17" />
        <span>{{ successMessage }}</span>
        <button @click="clearMessages">
          <X :size="16" />
        </button>
      </div>

      <!-- 搜索页面 -->
      <section v-if="currentPage === 'search'" class="page search-page">
        <div class="search-hero">
          <div class="hero-icon">
            <Languages :size="25" />
          </div>

          <h1>探索英语世界</h1>

          <p>输入一个英文单词，开始查询它的含义。</p>

          <div class="search-box-wrapper">
            <form class="search-box" @submit.prevent="searchWord()">
              <Search :size="21" class="search-box-icon" />

              <input
                ref="searchInput"
                v-model="searchText"
                type="text"
                placeholder="输入英文单词..."
                autocomplete="off"
                spellcheck="false"
                @input="handleSearchInput"
                @keydown="handleSearchKeydown"
                @focus="handleSearchInput"
              />

              <button type="submit" :disabled="isSearching">
                <RefreshCw v-if="isSearching" :size="17" class="spin" />
                <Search v-else :size="17" />
                <span>
                  {{ isSearching ? '查询中' : '查询' }}
                </span>
              </button>
            </form>

            <!-- 搜索建议列表 -->
            <div
              v-if="isSuggestionOpen && suggestions.length"
              class="suggestion-panel"
            >
              <button
                v-for="(item, index) in suggestions"
                :key="item.word.toLowerCase()"
                class="suggestion-item"
                :class="{
                  selected: selectedSuggestionIndex === index,
                }"
                @mousedown.prevent="selectSuggestion(item.word)"
                @mouseenter="selectedSuggestionIndex = index"
              >
                <span class="suggestion-word">
                  {{ item.word }}
                </span>

                <span
                  class="suggestion-source"
                  :class="{
                    'source-favorite': item.source === '收藏',
                    'source-history': item.source === '历史',
                  }"
                >
                  {{ item.source }}
                </span>
              </button>

              <div class="suggestion-footer">
                使用 ↑ ↓ 选择，Enter 查询，Esc 关闭
              </div>
            </div>
          </div>

          <!-- 搜索范围选择 -->
          <div class="search-options">
            <span class="search-options-label">搜索范围</span>

            <button
              type="button"
              class="search-option-button"
              :class="{ active: searchScope === 'all' }"
              @click="changeSearchScope('all')"
            >
              <Library :size="15" />
              全部词典
              <span class="search-option-count">
                {{ enabledDictionaries.length }}
              </span>
            </button>

            <button
              type="button"
              class="search-option-button"
              :class="{ active: searchScope === 'current' }"
              :disabled="!dictionaryInfo"
              @click="changeSearchScope('current')"
            >
              <BookOpen :size="15" />
              当前词典
            </button>
          </div>

          <div class="search-shortcut">
            <span>
              Enter 查询 · ↑ ↓ 选择建议 · Ctrl + K 快速搜索 · Esc 关闭
            </span>
          </div>
        </div>

        <!-- 词典状态 -->
        <div class="dictionary-card">
          <div class="dictionary-card-icon">
            <BookOpen :size="21" />
          </div>

          <div class="dictionary-card-info">
            <strong>
              {{ dictionaryInfo?.name ?? '尚未加载词典' }}
            </strong>

            <span>
              <template v-if="dictionaryInfo">
                {{ dictionaryInfo.entry_count.toLocaleString() }}
                条词典记录
              </template>

              <template v-else>
                尚未加载词典，请前往词典管理添加并启用本地 MDX 词典。
              </template>
            </span>
          </div>

          <button
            v-if="!dictionaryInfo"
            class="secondary-button"
            @click="currentPage = 'dictionaries'"
          >
            <Library :size="16" />
            前往词典管理
          </button>
        </div>
      </section>

      <section v-if="currentPage === 'dictionaries'" class="dictionary-page">
        <div class="dictionary-page-header">
          <div>
            <h2>词典管理</h2>
            <p>管理本地 MDX 词典文件，控制词典是否参与搜索。</p>
          </div>

          <button class="add-dictionary-button" @click="addDictionary">
            <Plus :size="16" />
            添加词典
          </button>
        </div>

        <div class="dictionary-summary">
          <div class="summary-card">
            <span class="summary-label">已启用 / 全部词典</span>
            <strong>
              {{ enabledDictionaries.length }} / {{ dictionaries.length }}
            </strong>
          </div>

          <div class="summary-card">
            <span class="summary-label">当前词典</span>
            <strong>
              {{ activeDictionary?.name ?? '尚未选择' }}
            </strong>
          </div>

          <div class="summary-card">
            <span class="summary-label">当前词条数量</span>
            <strong>
              {{
                activeDictionary
                  ? activeDictionary.entry_count.toLocaleString()
                  : '—'
              }}
            </strong>
          </div>
        </div>

        <div class="dictionary-list">
          <div v-if="dictionaries.length === 0" class="dictionary-empty">
            <BookOpen :size="36" />
            <h3>词典库还是空的</h3>
            <p>添加本地 MDX 文件，开始建立你的词典库。</p>

            <button class="add-dictionary-button" @click="addDictionary">
              添加第一部词典
            </button>
          </div>

          <div
            v-for="item in dictionaries"
            :key="item.path"
            class="dictionary-item"
            :class="{
              'dictionary-item-active': activeDictionaryPath === item.path,
            }"
          >
            <div class="dictionary-item-main">
              <div class="dictionary-icon">
                <BookOpen :size="21" />
              </div>

              <div class="dictionary-item-info">
                <div class="dictionary-item-title">
                  <strong>{{ item.name }}</strong>

                  <span
                    class="dictionary-status-badge"
                    :class="{
                      'status-enabled': item.enabled,
                      'status-disabled': !item.enabled,
                    }"
                  >
                    <span class="dictionary-status-dot"></span>
                    {{ item.enabled ? '已启用' : '已停用' }}
                  </span>
                </div>

                <p>{{ item.entry_count.toLocaleString() }} 个词条</p>

                <div class="dictionary-path">
                  {{ displayDictionaryPath(item.path) }}
                </div>
              </div>
            </div>

            <div class="dictionary-item-actions">
              <button
                v-if="item.enabled && activeDictionaryPath !== item.path"
                class="dictionary-switch-button"
                @click="activateDictionary(item.path)"
              >
                切换
              </button>

              <button
                class="dictionary-toggle-button"
                :class="{
                  'dictionary-enable-button': !item.enabled,
                }"
                @click="toggleDictionaryEnabled(item.path)"
              >
                <Power :size="14" />
                {{ item.enabled ? '停用' : '启用' }}
              </button>

              <button
                class="dictionary-remove-button"
                @click="deleteDictionary(item.path)"
              >
                <Trash2 :size="15" />
                移除
              </button>
            </div>
          </div>
        </div>
      </section>

      <!-- 搜索历史页面 -->
      <section v-else-if="currentPage === 'history'" class="page list-page">
        <div class="page-heading">
          <div>
            <h1>搜索历史</h1>
            <p>你之前查询过的所有单词都会保存在这里。</p>
          </div>

          <button
            class="danger-button"
            :disabled="historyList.length === 0"
            @click="clearHistory"
          >
            <Trash2 :size="16" />
            清空历史
          </button>
        </div>

        <div v-if="historyList.length" class="data-list">
          <button
            v-for="item in historyList"
            :key="item.id"
            class="data-row"
            @click="searchFromList(item.word)"
          >
            <div class="row-icon history-icon">
              <Clock3 :size="18" />
            </div>

            <div class="row-main">
              <strong>{{ item.word }}</strong>
              <span>{{ formatDate(item.searched_at) }}</span>
            </div>

            <ChevronRight :size="17" />
          </button>
        </div>

        <div v-else class="empty-state">
          <Clock3 :size="38" />
          <strong>暂无搜索历史</strong>
          <span>查询过的单词会显示在这里。</span>
          <button class="secondary-button" @click="switchPage('search')">
            开始查询
          </button>
        </div>
      </section>

      <!-- 收藏页面 -->
      <section v-else-if="currentPage === 'favorites'" class="page list-page">
        <div class="page-heading">
          <div>
            <h1>我的收藏</h1>
            <p>收藏你想要反复学习和记忆的单词。</p>
          </div>

          <span class="total-count"> 共 {{ favoriteList.length }} 个单词 </span>
        </div>

        <div v-if="favoriteList.length" class="data-list">
          <button
            v-for="item in favoriteList"
            :key="item.id"
            class="data-row"
            @click="searchFromList(item.word)"
          >
            <div class="row-icon favorite-icon">
              <Star :size="18" />
            </div>

            <div class="row-main">
              <strong>{{ item.word }}</strong>
              <span>收藏于 {{ formatDate(item.created_at) }}</span>
            </div>

            <ChevronRight :size="17" />
          </button>
        </div>

        <div v-else class="empty-state">
          <Bookmark :size="38" />
          <strong>暂无收藏单词</strong>
          <span>查询单词后，可以在释义窗口中添加收藏。</span>
          <button class="secondary-button" @click="switchPage('search')">
            开始查询
          </button>
        </div>
      </section>

      <!-- 单词学习页面 -->
      <section
        v-else-if="currentPage === 'learning'"
        class="page list-page learning-page"
      >
        <div class="page-heading">
          <div>
            <h1>单词学习</h1>
            <p>管理需要记忆和复习的英语单词。</p>
          </div>

          <div class="learning-heading-actions">
            <span class="total-count">
              共 {{ learningList.length }} 个学习单词
            </span>

            <button
              class="start-review-button"
              :disabled="dueLearningCount === 0"
              @click="startReview"
            >
              <RefreshCw :size="16" />
              开始复习
              <span v-if="dueLearningCount">
                {{ dueLearningCount }}
              </span>
            </button>
          </div>
        </div>

        <!-- 学习统计 -->
        <div class="learning-summary">
          <div class="learning-summary-card">
            <span class="learning-summary-label">待复习</span>
            <strong class="learning-summary-number due-number">
              {{ dueLearningCount }}
            </strong>
            <span class="learning-summary-description">
              需要再次记忆的单词
            </span>
          </div>

          <div class="learning-summary-card">
            <span class="learning-summary-label">学习中</span>
            <strong class="learning-summary-number studying-number">
              {{ studyingLearningCount }}
            </strong>
            <span class="learning-summary-description">
              尚未完全掌握的单词
            </span>
          </div>

          <div class="learning-summary-card">
            <span class="learning-summary-label">已经掌握</span>
            <strong class="learning-summary-number mastered-number">
              {{ masteredLearningCount }}
            </strong>
            <span class="learning-summary-description">
              熟悉程度达到最高级
            </span>
          </div>
        </div>

        <!-- 学习数据分析 -->
        <div class="learning-analytics">
          <!-- 核心统计 -->
          <div class="analytics-overview">
            <div class="analytics-stat-card">
              <span>今日复习</span>
              <strong>{{ todayReviewCount }}</strong>
              <small>今天完成的学习记录</small>
            </div>

            <div class="analytics-stat-card">
              <span>累计复习</span>
              <strong>{{ totalReviewCount }}</strong>
              <small>历史复习总次数</small>
            </div>

            <div class="analytics-stat-card">
              <span>连续学习</span>
              <strong>{{ reviewStreak }}<small> 天</small></strong>
              <small>连续有复习记录的天数</small>
            </div>

            <div class="analytics-stat-card">
              <span>复习单词</span>
              <strong>{{ reviewedWordCount }}</strong>
              <small>曾经复习过的不同单词</small>
            </div>
          </div>

          <!-- 每日复习趋势和熟悉程度分布 -->
          <div class="analytics-panels">
            <!-- 最近 7 天复习趋势 -->
            <section class="analytics-panel">
              <div class="analytics-panel-heading">
                <div>
                  <strong>最近 7 天复习趋势</strong>
                  <span>每天完成的复习次数</span>
                </div>
              </div>

              <div class="weekly-chart">
                <div
                  v-for="item in reviewDailyStats"
                  :key="item.date"
                  class="weekly-chart-column"
                >
                  <strong>{{ item.count }}</strong>

                  <div class="weekly-chart-track">
                    <div
                      class="weekly-chart-bar"
                      :style="{
                        height:
                          item.count === 0
                            ? '3px'
                            : `${(item.count / maxDailyReviewCount) * 100}%`,
                      }"
                    ></div>
                  </div>

                  <span>{{ item.label }}</span>
                </div>
              </div>
            </section>

            <!-- 熟悉程度分布 -->
            <section class="analytics-panel">
              <div class="analytics-panel-heading">
                <div>
                  <strong>熟悉程度分布</strong>
                  <span> 已掌握比例：{{ masteryPercentage }}% </span>
                </div>
              </div>

              <div class="familiarity-distribution">
                <div
                  v-for="item in familiarityDistribution"
                  :key="item.level"
                  class="distribution-row"
                >
                  <span class="distribution-level">
                    {{ item.level }}
                  </span>

                  <span class="distribution-label">
                    {{ item.label }}
                  </span>

                  <div class="distribution-track">
                    <div
                      class="distribution-fill"
                      :style="{
                        width: learningList.length
                          ? `${(item.count / learningList.length) * 100}%`
                          : '0%',
                      }"
                    ></div>
                  </div>

                  <strong>{{ item.count }}</strong>
                </div>
              </div>
            </section>
          </div>

          <!-- 最近复习记录 -->
          <section class="analytics-panel recent-review-panel">
            <div class="analytics-panel-heading">
              <div>
                <strong>最近复习记录</strong>
                <span>最近完成的 8 条学习记录</span>
              </div>

              <div class="review-history-heading-actions">
                <span class="recent-review-total">
                  共 {{ totalReviewCount }} 条
                </span>

                <button
                  class="review-history-link"
                  @click="switchPage('review-history')"
                >
                  查看全部
                  <ChevronRight :size="14" />
                </button>
              </div>
            </div>

            <div v-if="reviewHistory.length" class="recent-review-list">
              <div
                v-for="item in reviewHistory.slice(0, 8)"
                :key="item.id"
                class="recent-review-row"
              >
                <div class="recent-review-word">
                  <strong>{{ item.word }}</strong>
                  <span>
                    {{ familiarityLabels[item.familiarity] }}
                  </span>
                </div>

                <span class="recent-review-level">
                  熟悉程度 {{ item.familiarity }}/5
                </span>

                <span class="recent-review-time">
                  {{ item.reviewed_at }}
                </span>
              </div>
            </div>

            <div v-else class="analytics-empty">
              暂无复习历史。完成一次单词学习后，记录会显示在这里。
            </div>
          </section>
        </div>

        <!-- 学习记录筛选 -->
        <div class="learning-toolbar">
          <div class="learning-filter">
            <button
              :class="{ active: learningFilter === 'all' }"
              @click="setLearningFilter('all')"
            >
              全部
            </button>

            <button
              :class="{ active: learningFilter === 'due' }"
              @click="setLearningFilter('due')"
            >
              待复习
              <span v-if="dueLearningCount">
                {{ dueLearningCount }}
              </span>
            </button>

            <button
              :class="{ active: learningFilter === 'mastered' }"
              @click="setLearningFilter('mastered')"
            >
              已掌握
            </button>
          </div>
        </div>

        <!-- 学习记录批量操作 -->
        <div v-if="filteredLearningList.length" class="learning-bulk-toolbar">
          <label class="learning-select-all">
            <input
              type="checkbox"
              :checked="allVisibleLearningSelected"
              @change="toggleSelectAllVisibleLearning"
            />
            <span>全选当前列表</span>
          </label>

          <div class="learning-bulk-actions">
            <span> 已选择 {{ selectedLearningWords.length }} 条 </span>

            <button
              class="danger-button"
              :disabled="selectedLearningWords.length === 0"
              @click="removeSelectedLearningRecords"
            >
              <Trash2 :size="15" />
              删除选中
            </button>
          </div>
        </div>

        <!-- 学习记录列表 -->
        <div v-if="filteredLearningList.length" class="learning-list">
          <div
            v-for="item in filteredLearningList"
            :key="item.id"
            class="learning-row"
          >
            <input
              v-model="selectedLearningWords"
              class="learning-select-checkbox"
              type="checkbox"
              :value="item.word.toLocaleLowerCase()"
              :aria-label="`选择 ${item.word}`"
            />
            <button
              class="learning-word-button"
              @click="searchFromList(item.word)"
            >
              <div class="learning-word-icon">
                <GraduationCap :size="19" />
              </div>

              <div class="learning-word-main">
                <strong>{{ item.word }}</strong>

                <span>
                  熟悉程度：
                  {{ familiarityLabels[item.familiarity] }}
                </span>
              </div>
            </button>

            <div class="learning-row-info">
              <span
                class="learning-status"
                :class="{
                  'learning-status-due':
                    item.next_review_at <= getLocalDateString(),
                  'learning-status-mastered':
                    item.familiarity === 5 &&
                    item.next_review_at > getLocalDateString(),
                }"
              >
                {{
                  item.next_review_at <= getLocalDateString()
                    ? '待复习'
                    : item.familiarity === 5
                      ? '已掌握'
                      : '学习中'
                }}
              </span>

              <span class="learning-review-date">
                下次复习：{{ item.next_review_at }}
              </span>
            </div>

            <button
              class="learning-delete-button"
              title="删除学习记录"
              @click="removeLearningRecord(item.word)"
            >
              <Trash2 :size="16" />
            </button>
          </div>
        </div>

        <!-- 空状态 -->
        <div v-else class="empty-state">
          <GraduationCap :size="38" />

          <strong>
            {{
              learningList.length === 0
                ? '还没有学习记录'
                : learningFilter === 'due'
                  ? '目前没有待复习的单词'
                  : '还没有已经掌握的单词'
            }}
          </strong>

          <span>
            查询单词后，可以在释义窗口中设置熟悉程度，将单词加入学习计划。
          </span>

          <button class="secondary-button" @click="switchPage('search')">
            开始查询
          </button>
        </div>
      </section>

      <!-- 复习历史页面 -->
      <section
        v-else-if="currentPage === 'review-history'"
        class="page list-page review-history-page"
      >
        <!-- 页面标题 -->
        <div class="page-heading">
          <div>
            <h1>复习历史</h1>
            <p>查看每个单词的历史学习情况和熟悉程度变化。</p>
          </div>

          <div class="review-history-page-actions">
            <button
              class="danger-button"
              :disabled="reviewHistory.length === 0"
              @click="clearReviewHistory"
            >
              <Trash2 :size="15" />
              清空历史
            </button>

            <button
              class="export-history-button"
              :disabled="filteredReviewHistory.length === 0"
              @click="exportReviewHistoryCsv"
            >
              <Download :size="16" />
              导出 CSV
            </button>
          </div>
        </div>

        <!-- 历史统计 -->
        <div class="review-history-summary">
          <div class="review-history-stat">
            <span>历史记录总数</span>
            <strong>{{ totalReviewCount }}</strong>
          </div>

          <div class="review-history-stat">
            <span>不同单词数量</span>
            <strong>{{ reviewedWordCount }}</strong>
          </div>

          <div class="review-history-stat">
            <span>今日复习次数</span>
            <strong>{{ todayReviewCount }}</strong>
          </div>

          <div class="review-history-stat">
            <span>连续学习天数</span>
            <strong>{{ reviewStreak }} 天</strong>
          </div>
        </div>

        <!-- 筛选工具栏 -->
        <div class="review-history-filter-panel">
          <div class="review-history-filter-heading">
            <div>
              <strong>筛选复习记录</strong>
              <span>
                当前显示 {{ filteredReviewHistory.length }} /
                {{ reviewHistory.length }} 条记录
              </span>
            </div>

            <button
              class="review-history-reset"
              :disabled="!hasReviewHistoryFilters"
              @click="resetReviewHistoryFilters"
            >
              重置筛选
            </button>
          </div>

          <div class="review-history-filters">
            <!-- 单词搜索 -->
            <div class="review-history-filter-field word-filter-field">
              <label for="review-word-search">搜索单词</label>
              <div class="review-history-search-box">
                <Search :size="16" />

                <input
                  id="review-word-search"
                  v-model="reviewHistorySearch"
                  type="text"
                  placeholder="输入英文单词..."
                />

                <button
                  v-if="reviewHistorySearch"
                  title="清空搜索"
                  @click="reviewHistorySearch = ''"
                >
                  <X :size="14" />
                </button>
              </div>
            </div>

            <!-- 熟悉程度 -->
            <div class="review-history-filter-field">
              <label for="review-familiarity-filter"> 熟悉程度 </label>

              <select
                id="review-familiarity-filter"
                v-model="reviewHistoryFamiliarity"
              >
                <option value="all">全部等级</option>
                <option value="1">1 - 完全不熟悉</option>
                <option value="2">2 - 不太熟悉</option>
                <option value="3">3 - 基本认识</option>
                <option value="4">4 - 比较熟悉</option>
                <option value="5">5 - 已经掌握</option>
              </select>
            </div>

            <!-- 开始日期 -->
            <div class="review-history-filter-field">
              <label for="review-date-from">开始日期</label>

              <input
                id="review-date-from"
                v-model="reviewHistoryDateFrom"
                type="date"
                :max="reviewHistoryDateTo || undefined"
              />
            </div>

            <!-- 结束日期 -->
            <div class="review-history-filter-field">
              <label for="review-date-to">结束日期</label>

              <input
                id="review-date-to"
                v-model="reviewHistoryDateTo"
                type="date"
                :min="reviewHistoryDateFrom || undefined"
              />
            </div>
          </div>
        </div>

        <!-- 复习历史列表 -->
        <div v-if="filteredReviewHistory.length" class="review-history-list">
          <div
            v-for="item in filteredReviewHistory"
            :key="item.id"
            class="review-history-row"
          >
            <div class="review-history-row-icon">
              <CalendarCheck :size="18" />
            </div>

            <button
              class="review-history-word"
              @click="searchFromList(item.word)"
            >
              <strong>{{ item.word }}</strong>
              <span>点击查看单词释义</span>
            </button>

            <div class="review-history-familiarity">
              <span
                class="review-history-level"
                :class="`review-level-${item.familiarity}`"
              >
                {{ item.familiarity }}/5
              </span>

              <span>
                {{ familiarityLabels[item.familiarity] }}
              </span>
            </div>

            <div class="review-history-date">
              <span>复习时间</span>
              <strong>{{ formatDate(item.reviewed_at) }}</strong>
            </div>

            <button
              class="review-history-open"
              title="查看单词释义"
              @click="searchFromList(item.word)"
            >
              <ChevronRight :size="18" />
            </button>
          </div>
        </div>

        <!-- 空状态 -->
        <div v-else class="empty-state review-history-empty">
          <Clock3 :size="38" />

          <strong>
            {{
              reviewHistory.length === 0 ? '暂无复习历史' : '没有符合条件的记录'
            }}
          </strong>

          <span>
            {{
              reviewHistory.length === 0
                ? '完成单词学习或每日复习后，记录会显示在这里。'
                : '请尝试修改搜索关键词、熟悉程度或日期范围。'
            }}
          </span>

          <button
            v-if="hasReviewHistoryFilters"
            class="secondary-button"
            @click="resetReviewHistoryFilters"
          >
            重置筛选条件
          </button>

          <button
            v-else
            class="secondary-button"
            @click="switchPage('learning')"
          >
            返回单词学习
          </button>
        </div>
      </section>

      <!-- 设置页面 -->
<section
  v-else-if="currentPage === 'settings'"
  class="page settings-page"
>
  <div class="page-heading">
    <div>
      <h1>设置</h1>
      <p>管理个人学习数据与应用信息。</p>
    </div>
  </div>

  <!-- 数据备份与恢复 -->
  <div class="settings-card">
    <div class="settings-card-heading">
      <div class="settings-icon">
        <Download :size="20" />
      </div>

      <div>
        <strong>数据备份与恢复</strong>
        <span>备份和恢复你的个人词典学习数据</span>
      </div>
    </div>

    <div class="settings-detail">
      <span>备份内容</span>
      <strong>
        搜索历史、收藏、学习记录、复习历史
      </strong>
    </div>

    <div class="settings-detail">
      <span>备份格式</span>
      <strong>JSON</strong>
    </div>

    <div class="settings-detail">
      <span>恢复方式</span>
      <strong>覆盖当前四类数据</strong>
    </div>

    <div class="settings-actions">
      <button
        class="secondary-button"
        @click="exportAppBackup"
      >
        <Download :size="16" />
        导出备份
      </button>

      <button
        class="secondary-button"
        @click="restoreAppBackup"
      >
        <RefreshCw :size="16" />
        恢复备份
      </button>
    </div>
  </div>

  <!-- 应用信息：始终放在设置页面最下方 -->
  <div class="settings-card application-info-card">
    <div class="settings-card-heading">
      <div class="settings-icon">
        <BookOpen :size="20" />
      </div>

      <div>
        <strong>应用信息</strong>
        <span>小猪词典</span>
      </div>
    </div>

    <div class="settings-detail">
      <span>应用名称</span>
      <strong>小猪词典</strong>
    </div>

    <div class="settings-detail">
      <span>应用版本</span>
      <strong>0.1.0</strong>
    </div>
  </div>
</section>
    </main>

    <!-- 每日单词复习窗口 -->
    <div v-if="isReviewMode" class="review-overlay" @click.self="exitReview">
      <div class="review-dialog">
        <!-- 复习窗口顶部 -->
        <div class="review-header">
          <div>
            <strong>每日单词复习</strong>
            <span>WORD REVIEW</span>
          </div>

          <button
            class="review-close-button"
            title="退出复习"
            @click="exitReview"
          >
            <X :size="20" />
          </button>
        </div>

        <!-- 复习进度 -->
        <div v-if="currentReviewRecord" class="review-content">
          <div class="review-progress-info">
            <span>复习进度</span>
            <strong> {{ reviewIndex + 1 }} / {{ reviewQueue.length }} </strong>
          </div>

          <div class="review-progress-track">
            <div
              class="review-progress-fill"
              :style="{
                width: `${((reviewIndex + 1) / reviewQueue.length) * 100}%`,
              }"
            ></div>
          </div>

          <!-- 当前单词 -->
          <div class="review-word-card">
            <span class="review-instruction"> 请回忆这个单词的含义 </span>

            <h2>{{ currentReviewRecord.word }}</h2>

            <span class="review-word-number">
              第 {{ reviewIndex + 1 }} 个单词
            </span>
          </div>

          <!-- 尚未显示释义 -->
          <div v-if="!reviewAnswerVisible" class="review-reveal-area">
            <p>先尝试回忆单词的中文含义，再查看词典释义。</p>

            <button
              class="review-reveal-button"
              :disabled="isLoadingReviewDefinition"
              @click="showReviewAnswer"
            >
              <RefreshCw
                v-if="isLoadingReviewDefinition"
                :size="17"
                class="spin"
              />

              <BookOpen v-else :size="17" />

              {{ isLoadingReviewDefinition ? '正在加载释义...' : '显示释义' }}
            </button>
          </div>

          <!-- 已显示释义 -->
          <div v-else class="review-answer-area">
            <div class="review-answer-heading">
              <strong>词典释义</strong>
              <span> {{ reviewDefinitionResults.length }} 条词典记录 </span>
            </div>

            <div v-if="isLoadingReviewDefinition" class="review-loading">
              正在加载释义...
            </div>

            <div
              v-else-if="reviewDefinitionResults.length"
              class="review-definitions"
            >
              <article
                v-for="item in reviewDefinitionResults"
                :key="item.dictionary_path"
                class="review-definition-item"
              >
                <div class="review-dictionary-name">
                  <BookOpen :size="14" />
                  {{ item.dictionary_name }}
                </div>

                <div
                  class="definition-html"
                  v-html="sanitizeDefinition(item.definition)"
                ></div>
              </article>
            </div>

            <div v-else class="review-no-definition">
              没有从当前加载的词典中找到这个单词的释义。
              你仍然可以根据自己的记忆情况完成本次复习。
            </div>
          </div>

          <!-- 熟悉程度选择 -->
          <div v-if="reviewAnswerVisible" class="review-rating-area">
            <div class="review-rating-heading">
              <strong>你对这个单词的掌握程度如何？</strong>
              <span>选择后自动保存，并进入下一个单词。</span>
            </div>

            <div class="review-rating-options">
              <button
                v-for="level in [1, 2, 3, 4, 5]"
                :key="level"
                class="review-rating-button"
                :disabled="isSavingReview"
                @click="rateReview(level)"
              >
                <strong>{{ level }}</strong>
                <span>{{ familiarityLabels[level] }}</span>
                <small> {{ familiarityIntervals[level] }} 天后复习 </small>
              </button>
            </div>
          </div>
        </div>

        <!-- 异常空状态 -->
        <div v-else class="review-empty">
          <strong>本轮复习已经结束</strong>
          <button class="review-reveal-button" @click="isReviewMode = false">
            返回学习页面
          </button>
        </div>

        <!-- 底部说明 -->
        <div class="review-footer">
          <span> 每次选择熟悉程度后，系统会自动安排下一次复习。 </span>
        </div>
      </div>
    </div>

    <!-- 多词典释义模态窗口 -->

    <!-- 多词典释义模态窗口 -->
    <div
      v-if="isDefinitionOpen"
      class="modal-overlay"
      @click.self="closeDefinition"
    >
      <div
        class="definition-modal"
        role="dialog"
        aria-modal="true"
        aria-label="多词典释义"
      >
        <!-- 窗口顶部 -->
        <div class="modal-header">
          <div class="modal-title">
            <span>单词释义对照</span>
            <span class="modal-subtitle"> MULTI-DICTIONARY </span>
          </div>

          <button
            class="icon-button"
            aria-label="关闭"
            @click="closeDefinition"
          >
            <X :size="20" />
          </button>
        </div>

        <!-- 释义内容 -->
        <div class="definition-content">
          <!-- 单词标题 -->
          <div class="definition-word">
            <div>
              <h2>{{ currentDefinition?.headword ?? '当前筛选无结果' }}</h2>
              <span>
                {{ groupedSearchResults.length }} 个词头 ·
                {{ filteredSearchResults.length }} 条词典释义
              </span>
            </div>

            <button
              class="favorite-action"
              :class="{ favorited: isCurrentWordFavorite }"
              :disabled="!currentDefinition"
              @click="toggleFavorite"
            >
              <Star
                :size="18"
                :fill="isCurrentWordFavorite ? 'currentColor' : 'none'"
              />

              {{
                !currentDefinition
                  ? '暂无可收藏单词'
                  : isCurrentWordFavorite
                    ? '已收藏'
                    : '添加收藏'
              }}
            </button>
          </div>

          <!-- 单词学习状态 -->
          <div class="learning-control-panel">
            <div class="learning-control-heading">
              <div>
                <strong>学习状态</strong>
                <span>根据你对单词的掌握程度安排复习</span>
              </div>

              <span
                v-if="currentWordLearningRecord"
                class="learning-record-badge"
              >
                已加入学习
              </span>

              <span v-else class="learning-record-badge learning-record-new">
                尚未学习
              </span>
            </div>

            <div class="familiarity-options">
              <button
                v-for="level in [1, 2, 3, 4, 5]"
                :key="level"
                type="button"
                class="familiarity-button"
                :class="{
                  active: currentWordLearningRecord?.familiarity === level,
                }"
                :disabled="!currentDefinition"
                @click="saveLearningRecord(level)"
              >
                <strong>{{ level }}</strong>
                <span>{{ familiarityLabels[level] }}</span>
                <small> {{ familiarityIntervals[level] }} 天后复习 </small>
              </button>
            </div>

            <div v-if="currentWordLearningRecord" class="learning-record-info">
              当前熟悉程度：
              <strong>
                {{ familiarityLabels[currentWordLearningRecord.familiarity] }}
              </strong>

              <span>
                下次复习日期：
                {{ currentWordLearningRecord.next_review_at }}
              </span>
            </div>
          </div>

          <div class="definition-divider"></div>

          <!-- 多词典释义列表 -->
          <!-- 词典来源筛选 -->
          <!-- 搜索结果筛选 -->
          <div class="comparison-filter">
            <!-- 词典来源 -->
            <label for="dictionary-filter">词典来源</label>

            <select id="dictionary-filter" v-model="selectedDictionaryFilter">
              <option value="all">全部词典</option>

              <option
                v-for="dictionary in dictionaries"
                :key="dictionary.path"
                :value="dictionary.path"
              >
                {{ dictionary.name }}
              </option>
            </select>

            <!-- 匹配类型 -->
            <label for="match-type-filter">匹配类型</label>

            <select id="match-type-filter" v-model="selectedMatchType">
              <option value="all">全部类型</option>
              <option value="exact">精确匹配</option>
              <option value="prefix">前缀匹配</option>
              <option value="fuzzy">模糊匹配</option>
            </select>

            <!-- 筛选结果数量 -->
            <span class="comparison-filter-count">
              {{ filteredSearchResults.length }} 条释义
            </span>
          </div>

          <!-- 按词头分组的释义列表 -->
          <div v-if="groupedSearchResults.length" class="comparison-list">
            <section
              v-for="group in groupedSearchResults"
              :key="group.headword.toLocaleLowerCase()"
              class="comparison-group"
            >
              <!-- 分组词头 -->
              <div class="comparison-group-header">
                <strong>{{ group.headword }}</strong>

                <span> {{ group.results.length }} 个词典来源 </span>
              </div>

              <!-- 当前词头可用的词典来源 -->
              <div class="group-dictionary-tabs">
                <button
                  v-for="item in group.results"
                  :key="item.dictionary_path"
                  type="button"
                  class="group-dictionary-tab"
                  :class="{
                    active:
                      getSelectedResult(group)?.dictionary_path ===
                      item.dictionary_path,
                  }"
                  @click="selectGroupDictionary(group, item)"
                >
                  <BookOpen :size="14" />
                  {{ item.dictionary_name }}
                </button>
              </div>

              <!-- 当前选中的词典释义 -->
              <article
                v-if="getSelectedResult(group)"
                class="comparison-item selected-comparison-item"
              >
                <div class="comparison-header">
                  <div class="comparison-dictionary">
                    <BookOpen :size="17" />

                    <strong>
                      {{ getSelectedResult(group)?.dictionary_name }}
                    </strong>
                  </div>

                  <span
                    class="match-badge"
                    :class="{
                      'match-exact':
                        getSelectedResult(group)?.match_type === 'exact',
                      'match-prefix':
                        getSelectedResult(group)?.match_type === 'prefix',
                      'match-fuzzy':
                        getSelectedResult(group)?.match_type === 'fuzzy',
                    }"
                  >
                    {{
                      getSelectedResult(group)?.match_type === 'exact'
                        ? '精确匹配'
                        : getSelectedResult(group)?.match_type === 'prefix'
                          ? '前缀匹配'
                          : '模糊匹配'
                    }}

                    <template
                      v-if="getSelectedResult(group)?.distance !== null"
                    >
                      · 距离 {{ getSelectedResult(group)?.distance }}
                    </template>
                  </span>
                </div>

                <div
                  class="definition-html"
                  v-html="
                    sanitizeDefinition(
                      getSelectedResult(group)?.definition ?? '',
                    )
                  "
                ></div>
              </article>
            </section>
          </div>

          <!-- 筛选后没有结果 -->
          <div v-else class="comparison-empty">
            没有找到该词典来源对应的释义。
          </div>
        </div>

        <!-- 窗口底部 -->
        <div class="modal-footer">
          <span>
            当前单词：{{ currentDefinition?.headword ?? '-' }}
            · 当前词典：
            {{
              dictionaries.find((item) => item.path === currentDictionaryPath)
                ?.name ?? '未知词典'
            }}
            · 共 {{ groupedSearchResults.length }} 个词头
          </span>

          <button class="primary-button" @click="closeDefinition">
            <Check :size="16" />
            完成
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
/* --------------------------------------------------
   全局样式
-------------------------------------------------- */

:root {
  font-family: Inter, 'Segoe UI', 'Microsoft YaHei', sans-serif;

  color: #202938;
  background: #f6f7fb;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
}

body {
  width: 100%;
  height: 100vh;
  margin: 0;
  min-width: 800px;
  min-height: 0;
  overflow: hidden;
}

button,
input {
  font: inherit;
}

button {
  cursor: pointer;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

#app {
  width: 100%;
  height: 100vh;
  min-height: 0;
  overflow: hidden;
}

/* --------------------------------------------------
   主布局
-------------------------------------------------- */

.app-container {
  width: 100%;
  height: 100vh;
  min-height: 0;
  display: flex;
  overflow: hidden;
  background: #f6f7fb;
}

/* --------------------------------------------------
   左侧导航栏
-------------------------------------------------- */

.sidebar {
  width: 248px;
  min-width: 248px;
  height: 100vh;
  min-height: 0;
  flex-shrink: 0;

  display: flex;
  flex-direction: column;

  overflow: hidden;

  background: #172033;
  color: #fff;
  padding: 24px 15px 15px;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 10px;
  margin-bottom: 43px;
}

.brand-icon {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: #5269db;
  color: white;
}

.brand-name {
  font-size: 16px;
  font-weight: 700;
  letter-spacing: -0.3px;
}

.sidebar-section {
  margin-bottom: 31px;
}

.sidebar-label {
  padding: 0 12px;
  margin-bottom: 12px;
  color: #8994a9;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 1.3px;
}

.nav-item {
  width: 100%;
  height: 43px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 12px;
  margin: 3px 0;
  border: none;
  border-radius: 8px;
  color: #aeb8ca;
  background: transparent;
  text-align: left;
  font-size: 13px;
  transition: 0.18s ease;
}

.nav-item:hover {
  color: white;
  background: #26334b;
}

.nav-item.active {
  color: white;
  background: #34436a;
}

.nav-item.active svg {
  color: #aab8ff;
}

.nav-count {
  margin-left: auto;
  padding: 2px 7px;
  border-radius: 10px;
  color: #dbe3ff;
  background: #43537d;
  font-size: 10px;
}

.sidebar-bottom {
  margin-top: auto;
}

.sidebar-dictionary {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 13px 10px;
  border: 1px solid #344057;
  border-radius: 9px;
  background: #202c42;
}

.dictionary-status-icon {
  width: 31px;
  height: 31px;
  display: grid;
  place-items: center;
  border-radius: 7px;
  background: #34436a;
  color: #b8c5ff;
}

.dictionary-status-text {
  min-width: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 4px;
}

.status-title {
  color: #a1acc0;
  font-size: 10px;
}

.status-name {
  overflow: hidden;
  color: white;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-dot {
  width: 7px;
  height: 7px;
  flex-shrink: 0;
  border-radius: 50%;
  background: #7c8799;
}

.status-dot.connected {
  background: #36c98c;
}

.sidebar-version {
  padding: 16px 10px 2px;
  color: #78859c;
  font-size: 10px;
}

/* --------------------------------------------------
   主内容区域
-------------------------------------------------- */

.main-content {
  flex: 1;
  min-width: 0;
  height: 100vh;
  min-height: 0;

  display: flex;
  flex-direction: column;

  overflow: hidden;
}

.topbar {
  height: 65px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 35px;
  border-bottom: 1px solid #e9ebf1;
  background: #fff;
}

.breadcrumb {
  display: flex;
  align-items: center;
  gap: 9px;
  color: #929bad;
  font-size: 12px;
}

.breadcrumb strong {
  color: #344054;
  font-weight: 600;
}

.topbar-dictionary-button {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 14px;
  border: 1px solid #e0e4ed;
  border-radius: 7px;
  color: #46536a;
  background: #fff;
  font-size: 12px;
  transition: 0.18s;
}

.topbar-dictionary-button:hover {
  border-color: #667eea;
  color: #5269db;
}

.page {
  width: 100%;
  max-width: 1120px;
  flex: 1;
  min-height: 0;

  margin: 0 auto;
  padding: 40px 48px 60px;

  overflow-x: hidden;
  overflow-y: auto;
}

/* --------------------------------------------------
   消息提示
-------------------------------------------------- */

.message {
  display: flex;
  align-items: center;
  gap: 9px;
  margin: 15px 35px 0;
  padding: 11px 14px;
  border-radius: 7px;
  font-size: 12px;
}

.message span {
  flex: 1;
}

.message button {
  display: grid;
  place-items: center;
  border: none;
  background: transparent;
}

.message-error {
  color: #b42318;
  background: #fff0ee;
}

.message-success {
  color: #168451;
  background: #eaf8f0;
}

/* --------------------------------------------------
   搜索页面
-------------------------------------------------- */

.search-hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 30px 0 43px;
  text-align: center;
}

.hero-icon {
  width: 52px;
  height: 52px;
  display: grid;
  place-items: center;
  margin-bottom: 18px;
  border-radius: 15px;
  color: #5269db;
  background: #e9edff;
}

.search-hero h1 {
  margin: 0;
  color: #202a3a;
  font-size: 29px;
  font-weight: 750;
  letter-spacing: -1px;
}

.search-hero > p {
  margin: 12px 0 28px;
  color: #8791a2;
  font-size: 13px;
}

/* --------------------------------------------------
   搜索建议容器
-------------------------------------------------- */

.search-box-wrapper {
  position: relative;
  width: min(620px, 100%);
}

/* 搜索建议下拉面板 */
.suggestion-panel {
  position: absolute;
  top: calc(100% + 7px);
  left: 0;
  right: 0;
  z-index: 100;
  overflow: hidden;
  border: 1px solid #e2e6ef;
  border-radius: 10px;
  background: #fff;
  box-shadow: 0 12px 35px rgba(30, 43, 75, 0.12);
  text-align: left;
}

/* 单条建议 */
.suggestion-item {
  width: 100%;
  min-height: 43px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 17px;
  border: none;
  border-bottom: 1px solid #f0f1f5;
  color: #354052;
  background: white;
  text-align: left;
  transition: background 0.12s;
}

.suggestion-item:last-of-type {
  border-bottom: none;
}

.suggestion-item:hover,
.suggestion-item.selected {
  background: #f3f5ff;
}

.suggestion-word {
  font-size: 12px;
  font-weight: 600;
}

.suggestion-source {
  padding: 3px 7px;
  border-radius: 4px;
  color: #5269db;
  background: #edf0ff;
  font-size: 9px;
}

.suggestion-source.source-favorite {
  color: #b78325;
  background: #fff5dc;
}

.suggestion-source.source-history {
  color: #32836a;
  background: #eaf7f1;
}

/* 建议列表底部提示 */
.suggestion-footer {
  padding: 9px 17px;
  border-top: 1px solid #edf0f4;
  color: #9ba4b2;
  background: #fafbfc;
  font-size: 9px;
}

.search-box {
  width: min(620px, 100%);
  height: 58px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 7px 8px 7px 17px;
  border: 1px solid #e1e5ef;
  border-radius: 11px;
  background: white;
  box-shadow: 0 5px 22px rgba(30, 43, 75, 0.045);
  transition: 0.2s;
}

.search-box:focus-within {
  border-color: #8999ef;
  box-shadow: 0 0 0 3px rgba(82, 105, 219, 0.1);
}

.search-box-icon {
  flex-shrink: 0;
  color: #8e98aa;
}

.search-box input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  outline: none;
  color: #263247;
  background: transparent;
  font-size: 14px;
}

.search-box input::placeholder {
  color: #aab2c0;
}

.search-box button {
  height: 42px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 17px;
  border: none;
  border-radius: 7px;
  color: white;
  background: #5269db;
  font-size: 12px;
  font-weight: 600;
  transition: 0.18s;
}

.search-box button:hover {
  background: #4057c5;
}

.search-shortcut {
  margin-top: 13px;
  color: #a0a8b6;
  font-size: 10px;
}

/* --------------------------------------------------
   词典信息卡片
-------------------------------------------------- */

.dictionary-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 17px 19px;
  border: 1px solid #e8eaf1;
  border-radius: 10px;
  background: white;
}

.dictionary-card-icon {
  width: 42px;
  height: 42px;
  display: grid;
  place-items: center;
  border-radius: 9px;
  color: #5269db;
  background: #eef0ff;
}

.dictionary-card-info {
  min-width: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
}

.dictionary-card-info strong {
  overflow: hidden;
  color: #354052;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dictionary-card-info span {
  color: #939cad;
  font-size: 11px;
}

/* --------------------------------------------------
   通用按钮
-------------------------------------------------- */

.secondary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 9px 13px;
  border: 1px solid #e0e4ed;
  border-radius: 7px;
  color: #536078;
  background: white;
  font-size: 11px;
  font-weight: 600;
  transition: 0.18s;
}

.secondary-button:hover {
  border-color: #a6b2f1;
  color: #5269db;
  background: #fafaff;
}

.primary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 10px 17px;
  border: none;
  border-radius: 7px;
  color: white;
  background: #5269db;
  font-size: 12px;
  font-weight: 600;
}

.primary-button:hover {
  background: #4057c5;
}

.text-button {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 5px;
  border: none;
  color: #5269db;
  background: transparent;
  font-size: 11px;
}

.danger-button {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 9px 13px;
  border: 1px solid #f2d5d2;
  border-radius: 7px;
  color: #c24136;
  background: white;
  font-size: 11px;
}

.danger-button:hover:not(:disabled) {
  background: #fff5f4;
}

/* --------------------------------------------------
   最近搜索
-------------------------------------------------- */

.recent-section {
  margin-top: 42px;
}

.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.section-heading h2 {
  margin: 0;
  color: #344054;
  font-size: 15px;
}

.section-heading p {
  margin: 6px 0 0;
  color: #929bad;
  font-size: 11px;
}

.recent-list,
.data-list {
  overflow: hidden;
  border: 1px solid #e9ebf1;
  border-radius: 9px;
  background: white;
}

.recent-item {
  width: 100%;
  min-height: 52px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 17px;
  border: none;
  border-bottom: 1px solid #f0f1f5;
  color: #8993a3;
  background: white;
  text-align: left;
}

.recent-item:last-child {
  border-bottom: none;
}

.recent-item:hover {
  background: #fafbff;
}

.recent-word {
  flex: 1;
  color: #3a465a;
  font-size: 12px;
  font-weight: 600;
}

.recent-date {
  font-size: 10px;
}

/* --------------------------------------------------
   历史和收藏页面
-------------------------------------------------- */

.list-page,
.settings-page {
  padding-top: 43px;
}

.page-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 25px;
}

.page-heading h1 {
  margin: 0;
  color: #263247;
  font-size: 23px;
  letter-spacing: -0.5px;
}

.page-heading p {
  margin: 9px 0 0;
  color: #929bad;
  font-size: 12px;
}

.total-count {
  color: #8791a2;
  font-size: 11px;
}

.data-row {
  width: 100%;
  min-height: 70px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 11px 17px;
  border: none;
  border-bottom: 1px solid #f0f1f5;
  color: #929bad;
  background: white;
  text-align: left;
  transition: 0.15s;
}

.data-row:last-child {
  border-bottom: none;
}

.data-row:hover {
  background: #fafbff;
}

.row-icon {
  width: 36px;
  height: 36px;
  display: grid;
  flex-shrink: 0;
  place-items: center;
  border-radius: 8px;
}

.history-icon {
  color: #6075d9;
  background: #edf0ff;
}

.favorite-icon {
  color: #d49b2f;
  background: #fff5dc;
}

.row-main {
  min-width: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
}

.row-main strong {
  color: #354052;
  font-size: 13px;
}

.row-main span {
  color: #9ba4b2;
  font-size: 10px;
}

/* --------------------------------------------------
   空状态
-------------------------------------------------- */

.empty-state {
  min-height: 260px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  border: 1px dashed #dfe3ed;
  border-radius: 10px;
  color: #a0a9b8;
  background: rgba(255, 255, 255, 0.6);
  text-align: center;
}

.empty-state strong {
  color: #596579;
  font-size: 13px;
}

.empty-state span {
  font-size: 11px;
}

.empty-state .secondary-button {
  margin-top: 8px;
}

.compact-empty {
  min-height: 120px;
  border: 1px dashed #e2e5ed;
}

/* --------------------------------------------------
   设置页面
-------------------------------------------------- */

.settings-card {
  margin-bottom: 18px;
  padding: 21px;
  border: 1px solid #e8eaf1;
  border-radius: 10px;
  background: white;
}

.settings-card-heading {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 22px;
}

.settings-icon {
  width: 39px;
  height: 39px;
  display: grid;
  place-items: center;
  border-radius: 9px;
  color: #5269db;
  background: #edf0ff;
}

.settings-card-heading > div:last-child {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.settings-card-heading strong {
  color: #354052;
  font-size: 13px;
}

.settings-card-heading span {
  color: #969faf;
  font-size: 10px;
}

.settings-detail {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  padding: 13px 0;
  border-top: 1px solid #f0f1f5;
}

.settings-detail > span {
  flex-shrink: 0;
  color: #8d97a8;
  font-size: 11px;
}

.settings-detail strong {
  max-width: 70%;
  color: #46536a;
  font-size: 11px;
  font-weight: 500;
  text-align: right;
  overflow-wrap: anywhere;
}

.settings-actions {
  display: flex;
  gap: 10px;
  margin-top: 18px;
}

/* --------------------------------------------------
   释义模态窗口
-------------------------------------------------- */

.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 30px;
  background: rgba(18, 27, 43, 0.58);
  backdrop-filter: blur(3px);
}

.definition-modal {
  width: min(760px, 100%);
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: 13px;
  background: white;
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.2);
  animation: modal-in 0.18s ease-out;
}

@keyframes modal-in {
  from {
    opacity: 0;
    transform: translateY(9px) scale(0.99);
  }

  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.modal-header {
  height: 62px;
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: space-between;
  padding: 0 23px;
  border-bottom: 1px solid #edf0f4;
}

.modal-title {
  display: flex;
  align-items: center;
  gap: 11px;
  color: #344054;
  font-size: 13px;
  font-weight: 700;
}

.modal-subtitle {
  color: #a1aabb;
  font-size: 9px;
  font-weight: 500;
  letter-spacing: 1px;
}

.icon-button {
  width: 33px;
  height: 33px;
  display: grid;
  place-items: center;
  border: none;
  border-radius: 7px;
  color: #8993a3;
  background: transparent;
}

.icon-button:hover {
  color: #344054;
  background: #f0f2f6;
}

.definition-content {
  overflow-y: auto;
  padding: 26px 30px 32px;
}

.definition-word {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
}

.definition-word h2 {
  margin: 0;
  color: #27344a;
  font-size: 27px;
  letter-spacing: -0.5px;
}

.definition-word span {
  display: inline-block;
  margin-top: 7px;
  color: #9aa3b2;
  font-size: 10px;
}

.favorite-action {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 9px 12px;
  border: 1px solid #e4e7ee;
  border-radius: 7px;
  color: #68758a;
  background: white;
  font-size: 11px;
}

.favorite-action:hover {
  border-color: #e6c16f;
  color: #c18b26;
}

.favorite-action.favorited {
  border-color: #f0dfb3;
  color: #bd8a2d;
  background: #fff9e9;
}

.definition-divider {
  height: 1px;
  margin: 23px 0;
  background: #edf0f4;
}

.definition-html {
  color: #354052;
  font-size: 13px;
  line-height: 1.85;
  overflow-wrap: anywhere;
}

.definition-html p {
  margin: 9px 0;
}

.definition-html b,
.definition-html strong {
  color: #283a5a;
  font-weight: 700;
}

.definition-html h1,
.definition-html h2,
.definition-html h3,
.definition-html h4 {
  margin: 18px 0 9px;
  color: #34466a;
  font-size: 15px;
}

.definition-html ul,
.definition-html ol {
  padding-left: 24px;
}

.definition-html li {
  margin: 5px 0;
}

.definition-html a {
  color: #5269db;
  text-decoration: none;
}

.definition-html a:hover {
  text-decoration: underline;
}

.definition-html img {
  max-width: 100%;
  height: auto;
}

.definition-html table {
  width: 100%;
  border-collapse: collapse;
  margin: 13px 0;
  font-size: 12px;
}

.definition-html th,
.definition-html td {
  padding: 8px 10px;
  border: 1px solid #e4e7ee;
  text-align: left;
}

.definition-html th {
  background: #f6f7fb;
}

.definition-html audio {
  max-width: 100%;
}

.modal-footer {
  min-height: 62px;
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: space-between;
  gap: 15px;
  padding: 10px 23px;
  border-top: 1px solid #edf0f4;
  background: #fcfcfd;
}

.modal-footer > span {
  overflow: hidden;
  color: #9aa3b2;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* --------------------------------------------------
   加载动画
-------------------------------------------------- */

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* --------------------------------------------------
   滚动条
-------------------------------------------------- */

::-webkit-scrollbar {
  width: 7px;
  height: 7px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  border-radius: 10px;
  background: #d8dce5;
}

::-webkit-scrollbar-thumb:hover {
  background: #b9c0ce;
}

/* --------------------------------------------------
   响应式布局
-------------------------------------------------- */

@media (max-width: 1050px) {
  .sidebar {
    width: 215px;
    min-width: 215px;
  }

  .page {
    padding-right: 30px;
    padding-left: 30px;
  }
}

@media (max-width: 850px) {
  .sidebar {
    width: 190px;
    min-width: 190px;
    padding-right: 10px;
    padding-left: 10px;
  }

  .brand-name {
    font-size: 14px;
  }

  .page {
    padding-right: 22px;
    padding-left: 22px;
  }
}

/* ==============================
   Dictionary Management
   ============================== */

.dictionary-page {
  width: 100%;
  max-width: 1100px;
  margin: 0 auto;
  padding: 30px 36px;
}

.dictionary-page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 28px;
}

.dictionary-page-header h2 {
  margin: 0;
  color: #253047;
  font-size: 22px;
  font-weight: 700;
}

.dictionary-page-header p {
  margin: 8px 0 0;
  color: #8993a4;
  font-size: 12px;
}

.add-dictionary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 10px 16px;
  border: none;
  border-radius: 7px;
  color: white;
  background: #5369d9;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}

.add-dictionary-button:hover {
  background: #4055c4;
}

.dictionary-summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 24px;
}

.summary-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  padding: 20px;
  border: 1px solid #e9ecf2;
  border-radius: 10px;
  background: white;
}

.summary-label {
  color: #8b95a6;
  font-size: 11px;
}

.summary-card strong {
  overflow: hidden;
  color: #303b52;
  font-size: 19px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dictionary-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.dictionary-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  min-height: 260px;
  border: 1px dashed #dce1eb;
  border-radius: 10px;
  color: #9aa4b4;
  background: white;
}

.dictionary-empty h3 {
  margin: 0;
  color: #47536a;
  font-size: 15px;
}

.dictionary-empty p {
  margin: 0 0 8px;
  font-size: 11px;
}

.dictionary-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 19px 20px;
  border: 1px solid #e9ecf2;
  border-radius: 10px;
  background: white;
  transition:
    border-color 0.2s,
    background 0.2s;
}

.dictionary-item-active {
  border-color: #8998e9;
  background: #fafbff;
}

.dictionary-item-main {
  display: flex;
  align-items: flex-start;
  gap: 15px;
  min-width: 0;
  flex: 1;
}

.dictionary-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  flex-shrink: 0;
  border-radius: 9px;
  color: #5369d9;
  background: #eef0ff;
}

.dictionary-item-info {
  min-width: 0;
}

.dictionary-item-title {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 9px;
}

.dictionary-item-title strong {
  color: #354057;
  font-size: 13px;
}

.dictionary-active-badge {
  padding: 3px 7px;
  border-radius: 4px;
  color: #4c60c5;
  background: #edf0ff;
  font-size: 9px;
}

.dictionary-item-info p {
  margin: 7px 0;
  color: #8993a4;
  font-size: 10px;
}

.dictionary-path {
  overflow: hidden;
  max-width: 650px;
  color: #a0a8b5;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dictionary-item-actions {
  display: flex;
  align-items: center;
  gap: 9px;
  flex-shrink: 0;
}

.dictionary-switch-button,
.dictionary-remove-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 8px 12px;
  border: 1px solid #e5e8ef;
  border-radius: 6px;
  background: white;
  font-size: 10px;
  cursor: pointer;
}

.dictionary-switch-button {
  color: #5369d9;
}

.dictionary-switch-button:hover {
  border-color: #5369d9;
  background: #f5f6ff;
}

.dictionary-remove-button {
  color: #bd6670;
}

.dictionary-remove-button:hover {
  border-color: #e3aeb3;
  background: #fff7f7;
}

@media (max-width: 760px) {
  .dictionary-page {
    padding: 22px 16px;
  }

  .dictionary-summary {
    grid-template-columns: 1fr;
  }

  .dictionary-item {
    align-items: flex-start;
    flex-direction: column;
  }

  .dictionary-item-actions {
    align-self: flex-end;
  }
}

/* --------------------------------------------------
   搜索范围选择
-------------------------------------------------- */

.search-options {
  width: min(620px, 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-wrap: wrap;
  gap: 9px;
  margin-top: 17px;
}

.search-options-label {
  margin-right: 3px;
  color: #8993a3;
  font-size: 11px;
}

.search-option-button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 11px;
  border: 1px solid #e2e6ef;
  border-radius: 7px;
  color: #69758a;
  background: white;
  font-size: 11px;
  transition: 0.18s;
}

.search-option-button:hover {
  border-color: #a6b2f1;
  color: #5269db;
}

.search-option-button.active {
  border-color: #cbd3ff;
  color: #5269db;
  background: #f0f2ff;
}

.search-option-count {
  min-width: 17px;
  padding: 2px 5px;
  border-radius: 10px;
  color: #5269db;
  background: #e1e6ff;
  font-size: 9px;
}

/* --------------------------------------------------
   多词典释义对照
-------------------------------------------------- */

/* --------------------------------------------------
   分组内词典切换
-------------------------------------------------- */

.group-dictionary-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 12px 0;
}

.group-dictionary-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 11px;
  border: 1px solid #e3e7ef;
  border-radius: 6px;
  color: #69758a;
  background: #fff;
  font-size: 10px;
  transition: 0.18s;
}

.group-dictionary-tab:hover {
  border-color: #a6b2f1;
  color: #5269db;
  background: #f8f9ff;
}

.group-dictionary-tab.active {
  border-color: #5269db;
  color: #5269db;
  background: #f0f2ff;
  font-weight: 650;
}

.selected-comparison-item {
  margin-top: 0;
}

.comparison-list {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.comparison-item {
  overflow: hidden;
  padding: 18px 19px;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: #fff;
}

.comparison-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px;
  padding-bottom: 12px;
  border-bottom: 1px solid #edf0f4;
}

.comparison-dictionary {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #5269db;
  font-size: 12px;
}

.comparison-dictionary strong {
  color: #344054;
  font-weight: 650;
}

.comparison-headword {
  margin: 15px 0 12px;
  color: #27344a;
  font-size: 17px;
  font-weight: 700;
}

.match-badge {
  padding: 4px 8px;
  border-radius: 5px;
  font-size: 9px;
  font-weight: 600;
}

.match-exact {
  color: #168451;
  background: #eaf8f0;
}

.match-prefix {
  color: #5269db;
  background: #edf0ff;
}

.match-fuzzy {
  color: #a56b19;
  background: #fff5df;
}

/* 多词典释义之间的间距 */

.comparison-item .definition-html {
  margin-top: 10px;
}

.comparison-item .definition-html > :first-child {
  margin-top: 0;
}

.comparison-item .definition-html > :last-child {
  margin-bottom: 0;
}

/* --------------------------------------------------
   释义窗口
-------------------------------------------------- */

.definition-modal {
  width: min(900px, 100%);
}

.definition-content {
  min-height: 100px;
}

@media (max-width: 900px) {
  .search-options {
    gap: 7px;
  }

  .comparison-item {
    padding: 14px;
  }

  .definition-modal {
    max-height: 90vh;
  }
}

/* --------------------------------------------------
   多词典结果分组与来源筛选
-------------------------------------------------- */

.comparison-filter {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 20px;
  padding: 12px 14px;
  border: 1px solid #e8eaf1;
  border-radius: 8px;
  background: #fafbfe;
}

.comparison-filter label {
  flex-shrink: 0;
  color: #69758a;
  font-size: 11px;
}

.comparison-filter select {
  max-width: 260px;
  padding: 7px 28px 7px 10px;
  border: 1px solid #e0e4ed;
  border-radius: 6px;
  outline: none;
  color: #46536a;
  background: white;
  font-size: 11px;
}

.comparison-filter select:focus {
  border-color: #8999ef;
}

.comparison-filter-count {
  margin-left: auto;
  color: #929bad;
  font-size: 10px;
  white-space: nowrap;
}

.comparison-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 22px;
}

.comparison-group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 14px;
  border-left: 3px solid #5269db;
  border-radius: 5px;
  background: #f2f4ff;
}

.comparison-group-header strong {
  color: #34466a;
  font-size: 15px;
}

.comparison-group-header span {
  color: #8993a4;
  font-size: 10px;
}

.comparison-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 120px;
  border: 1px dashed #dfe3ed;
  border-radius: 9px;
  color: #929bad;
  background: #fafbfe;
  font-size: 12px;
}

.comparison-group .comparison-item {
  margin-left: 12px;
}

@media (max-width: 600px) {
  .comparison-filter {
    flex-wrap: wrap;
  }

  .comparison-filter select {
    max-width: 100%;
    flex: 1;
  }

  .comparison-filter-count {
    margin-left: 0;
  }

  .comparison-group .comparison-item {
    margin-left: 0;
  }
}

/* --------------------------------------------------
   单词学习页面
-------------------------------------------------- */

.learning-page {
  padding-bottom: 35px;
}

.learning-summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 15px;
  margin-bottom: 25px;
}

.learning-summary-card {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding: 19px;
  border: 1px solid #e8eaf1;
  border-radius: 10px;
  background: white;
}

.learning-summary-label {
  color: #8490a3;
  font-size: 11px;
}

.learning-summary-number {
  font-size: 27px;
  line-height: 1.2;
}

.due-number {
  color: #d28b35;
}

.studying-number {
  color: #5269db;
}

.mastered-number {
  color: #23966b;
}

.learning-summary-description {
  color: #a0a9b8;
  font-size: 10px;
}

.learning-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 15px;
}

.learning-filter {
  display: flex;
  align-items: center;
  gap: 7px;
}

.learning-filter button {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 8px 13px;
  border: 1px solid #e4e7ee;
  border-radius: 7px;
  color: #69758a;
  background: white;
  font-size: 11px;
}

.learning-filter button:hover {
  background: #f7f8fc;
}

.learning-filter button.active {
  border-color: #cbd3ff;
  color: #5269db;
  background: #edf0ff;
}

.learning-filter button span {
  padding: 2px 6px;
  border-radius: 8px;
  color: #a76b24;
  background: #fff0d9;
  font-size: 9px;
}

.learning-list {
  overflow: hidden;
  border: 1px solid #e9ebf1;
  border-radius: 9px;
  background: white;
}

.learning-row {
  min-height: 76px;
  display: flex;
  align-items: center;
  gap: 17px;
  padding: 11px 17px;
  border-bottom: 1px solid #f0f1f5;
}

.learning-row:last-child {
  border-bottom: none;
}

.learning-word-button {
  min-width: 0;
  display: flex;
  flex: 1;
  align-items: center;
  gap: 13px;
  padding: 0;
  border: none;
  background: transparent;
  text-align: left;
}

.learning-word-button:hover .learning-word-main strong {
  color: #5269db;
}

.learning-word-icon {
  width: 37px;
  height: 37px;
  display: grid;
  flex-shrink: 0;
  place-items: center;
  border-radius: 8px;
  color: #5269db;
  background: #edf0ff;
}

.learning-word-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.learning-word-main strong {
  color: #354052;
  font-size: 13px;
}

.learning-word-main span {
  color: #9ba4b2;
  font-size: 10px;
}

.learning-row-info {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 6px;
}

.learning-status {
  padding: 4px 8px;
  border-radius: 5px;
  color: #5269db;
  background: #edf0ff;
  font-size: 9px;
}

.learning-status-due {
  color: #b87524;
  background: #fff3df;
}

.learning-status-mastered {
  color: #168451;
  background: #eaf8f0;
}

.learning-review-date {
  color: #929bad;
  font-size: 10px;
}

.learning-delete-button {
  width: 32px;
  height: 32px;
  display: grid;
  flex-shrink: 0;
  place-items: center;
  border: none;
  border-radius: 6px;
  color: #a0a8b5;
  background: transparent;
}

.learning-delete-button:hover {
  color: #c24136;
  background: #fff1ef;
}

/* --------------------------------------------------
   释义窗口中的学习状态
-------------------------------------------------- */

.learning-control-panel {
  margin-top: 22px;
  padding: 17px;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: #fafbfe;
}

.learning-control-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 15px;
}

.learning-control-heading > div {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.learning-control-heading strong {
  color: #354052;
  font-size: 12px;
}

.learning-control-heading > div span {
  color: #929bad;
  font-size: 10px;
}

.learning-record-badge {
  padding: 5px 9px;
  border-radius: 5px;
  color: #168451;
  background: #eaf8f0;
  font-size: 9px;
  white-space: nowrap;
}

.learning-record-new {
  color: #8490a3;
  background: #edf0f4;
}

.familiarity-options {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 8px;
}

.familiarity-button {
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 7px;
  padding: 11px 5px;
  border: 1px solid #e3e6ee;
  border-radius: 7px;
  color: #68758a;
  background: white;
  transition: 0.15s ease;
}

.familiarity-button:hover:not(:disabled) {
  border-color: #aab7ff;
  background: #f6f7ff;
}

.familiarity-button.active {
  border-color: #7889ed;
  color: #5269db;
  background: #edf0ff;
}

.familiarity-button strong {
  font-size: 17px;
}

.familiarity-button span {
  font-size: 10px;
  white-space: nowrap;
}

.familiarity-button small {
  color: #9ba4b2;
  font-size: 9px;
  white-space: nowrap;
}

.familiarity-button.active small {
  color: #687bd9;
}

.learning-record-info {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 13px;
  padding-top: 12px;
  border-top: 1px solid #e9ebf1;
  color: #929bad;
  font-size: 10px;
}

.learning-record-info strong {
  color: #5269db;
}

.learning-record-info span {
  margin-left: auto;
  color: #68758a;
  font-size: 10px;
}

@media (max-width: 900px) {
  .learning-summary {
    gap: 9px;
  }

  .learning-summary-card {
    padding: 14px;
  }

  .familiarity-options {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (max-width: 600px) {
  .learning-summary {
    grid-template-columns: 1fr;
  }

  .learning-row {
    gap: 9px;
    padding: 11px;
  }

  .learning-row-info {
    display: none;
  }

  .familiarity-options {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .learning-record-info span {
    margin-left: 0;
  }
}

/* --------------------------------------------------
   每日单词复习窗口
-------------------------------------------------- */

.learning-heading-actions {
  display: flex;
  align-items: center;
  gap: 13px;
}

.start-review-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 9px 15px;
  border: none;
  border-radius: 7px;
  color: white;
  background: #5369d9;
  font-size: 11px;
  font-weight: 600;
  transition: background 0.18s;
}

.start-review-button:hover:not(:disabled) {
  background: #4055c4;
}

.start-review-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.start-review-button span {
  min-width: 19px;
  padding: 2px 5px;
  border-radius: 10px;
  color: #5369d9;
  background: white;
  font-size: 9px;
}

.review-overlay {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 25px;
  background: rgba(15, 23, 42, 0.62);
  backdrop-filter: blur(4px);
}

.review-dialog {
  width: min(760px, 100%);
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid #e7eaf1;
  border-radius: 13px;
  background: #fff;
  box-shadow: 0 20px 70px rgba(0, 0, 0, 0.2);
}

.review-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 19px 23px;
  border-bottom: 1px solid #edf0f4;
}

.review-header > div {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.review-header strong {
  color: #303b52;
  font-size: 14px;
}

.review-header span {
  color: #a0a8b5;
  font-size: 9px;
  letter-spacing: 1px;
}

.review-close-button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border: none;
  border-radius: 7px;
  color: #8490a3;
  background: transparent;
}

.review-close-button:hover {
  color: #c24136;
  background: #fff1ef;
}

.review-content {
  overflow-y: auto;
  padding: 23px;
}

.review-progress-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 9px;
  color: #8993a4;
  font-size: 11px;
}

.review-progress-info strong {
  color: #5369d9;
  font-size: 12px;
}

.review-progress-track {
  height: 6px;
  overflow: hidden;
  border-radius: 5px;
  background: #edf0f5;
}

.review-progress-fill {
  height: 100%;
  border-radius: inherit;
  background: #5369d9;
  transition: width 0.2s ease;
}

.review-word-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 15px;
  min-height: 205px;
  margin: 22px 0;
  padding: 25px;
  border: 1px solid #e6e9f2;
  border-radius: 10px;
  background: #fafbff;
}

.review-instruction {
  color: #929bad;
  font-size: 11px;
}

.review-word-card h2 {
  margin: 0;
  color: #303b52;
  font-size: 34px;
  font-weight: 700;
  letter-spacing: -0.5px;
}

.review-word-number {
  color: #a0a8b5;
  font-size: 10px;
}

.review-reveal-area {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  padding: 8px 0 20px;
}

.review-reveal-area p {
  margin: 0;
  color: #929bad;
  font-size: 11px;
  text-align: center;
}

.review-reveal-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 11px 22px;
  border: none;
  border-radius: 7px;
  color: white;
  background: #5369d9;
  font-size: 12px;
  font-weight: 600;
}

.review-reveal-button:hover:not(:disabled) {
  background: #4055c4;
}

.review-answer-area {
  margin-top: 17px;
}

.review-answer-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 12px;
}

.review-answer-heading strong {
  color: #354052;
  font-size: 12px;
}

.review-answer-heading span {
  color: #929bad;
  font-size: 10px;
}

.review-definitions {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 300px;
  overflow-y: auto;
}

.review-definition-item {
  padding: 14px;
  border: 1px solid #e9ebf1;
  border-radius: 8px;
  background: #fff;
}

.review-dictionary-name {
  display: flex;
  align-items: center;
  gap: 7px;
  padding-bottom: 10px;
  border-bottom: 1px solid #edf0f4;
  color: #5369d9;
  font-size: 11px;
  font-weight: 600;
}

.review-definition-item .definition-html {
  padding-top: 10px;
}

.review-loading,
.review-no-definition {
  padding: 17px;
  border-radius: 7px;
  color: #929bad;
  background: #f7f8fb;
  font-size: 11px;
  line-height: 1.8;
}

.review-rating-area {
  margin-top: 22px;
  padding-top: 19px;
  border-top: 1px solid #edf0f4;
}

.review-rating-heading {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
}

.review-rating-heading strong {
  color: #354052;
  font-size: 12px;
}

.review-rating-heading span {
  color: #929bad;
  font-size: 10px;
}

.review-rating-options {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 8px;
}

.review-rating-button {
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 7px;
  padding: 12px 5px;
  border: 1px solid #e3e6ee;
  border-radius: 7px;
  color: #68758a;
  background: white;
  transition: 0.15s ease;
}

.review-rating-button:hover:not(:disabled) {
  border-color: #aab7ff;
  color: #5269db;
  background: #f6f7ff;
}

.review-rating-button strong {
  font-size: 18px;
}

.review-rating-button span {
  font-size: 10px;
  white-space: nowrap;
}

.review-rating-button small {
  color: #9ba4b2;
  font-size: 9px;
  white-space: nowrap;
}

.review-rating-button:disabled {
  cursor: wait;
}

.review-footer {
  padding: 13px 22px;
  border-top: 1px solid #edf0f4;
  color: #a0a8b5;
  background: #fafbfc;
  font-size: 10px;
  text-align: center;
}

.review-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 18px;
  padding: 55px 20px;
  color: #5369d9;
}

.review-empty strong {
  color: #354052;
  font-size: 14px;
}

@media (max-width: 600px) {
  .learning-heading-actions {
    align-items: flex-end;
    flex-direction: column;
    gap: 8px;
  }

  .review-overlay {
    padding: 10px;
  }

  .review-dialog {
    max-height: 95vh;
  }

  .review-content {
    padding: 15px;
  }

  .review-rating-options {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .review-word-card h2 {
    font-size: 27px;
  }
}

/* --------------------------------------------------
   学习数据分析
-------------------------------------------------- */

.learning-analytics {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-bottom: 25px;
}

/* 核心统计卡片 */

.analytics-overview {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.analytics-stat-card {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding: 16px;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: white;
}

.analytics-stat-card > span {
  color: #8490a3;
  font-size: 10px;
}

.analytics-stat-card > strong {
  color: #354052;
  font-size: 25px;
  line-height: 1.2;
}

.analytics-stat-card > strong small {
  font-size: 12px;
  font-weight: 500;
}

.analytics-stat-card > small {
  color: #a0a9b8;
  font-size: 9px;
}

/* 统计面板 */

.analytics-panels {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.analytics-panel {
  min-width: 0;
  padding: 17px;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: white;
}

.analytics-panel-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 19px;
}

.analytics-panel-heading > div {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.analytics-panel-heading strong {
  color: #354052;
  font-size: 12px;
}

.analytics-panel-heading span {
  color: #9ba4b2;
  font-size: 9px;
}

/* 最近七天柱状图 */

.weekly-chart {
  height: 145px;
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 10px;
}

.weekly-chart-column {
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  gap: 7px;
}

.weekly-chart-column > strong {
  color: #7d8798;
  font-size: 10px;
  font-weight: 500;
}

.weekly-chart-track {
  width: 100%;
  height: 100px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  overflow: hidden;
  border-radius: 5px;
  background: #f2f4f8;
}

.weekly-chart-bar {
  width: min(24px, 70%);
  min-height: 3px;
  border-radius: 5px 5px 2px 2px;
  background: #7889e9;
  transition: height 0.2s ease;
}

.weekly-chart-column > span {
  color: #929bad;
  font-size: 9px;
}

/* 熟悉程度分布 */

.familiarity-distribution {
  display: flex;
  flex-direction: column;
  gap: 15px;
}

.distribution-row {
  display: grid;
  grid-template-columns: 22px 75px minmax(0, 1fr) 24px;
  align-items: center;
  gap: 8px;
}

.distribution-level {
  width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: 5px;
  color: #5369d9;
  background: #edf0ff;
  font-size: 10px;
  font-weight: 600;
}

.distribution-label {
  overflow: hidden;
  color: #7f899a;
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.distribution-track {
  height: 7px;
  overflow: hidden;
  border-radius: 5px;
  background: #f0f2f6;
}

.distribution-fill {
  height: 100%;
  border-radius: 5px;
  background: #7889e9;
  transition: width 0.2s ease;
}

.distribution-row > strong {
  color: #647086;
  font-size: 10px;
  text-align: right;
}

/* 最近复习记录 */

.recent-review-panel {
  width: 100%;
}

.recent-review-total {
  white-space: nowrap;
}

.recent-review-list {
  display: flex;
  flex-direction: column;
}

.recent-review-row {
  min-height: 48px;
  display: flex;
  align-items: center;
  gap: 15px;
  padding: 9px 3px;
  border-top: 1px solid #f0f1f5;
}

.recent-review-word {
  min-width: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 5px;
}

.recent-review-word strong {
  overflow: hidden;
  color: #354052;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-review-word span {
  color: #9ba4b2;
  font-size: 9px;
}

.recent-review-level {
  flex-shrink: 0;
  color: #6578d8;
  font-size: 9px;
}

.recent-review-time {
  flex-shrink: 0;
  color: #9ba4b2;
  font-size: 9px;
}

.analytics-empty {
  padding: 20px 10px;
  color: #9ba4b2;
  font-size: 10px;
  text-align: center;
}

@media (max-width: 900px) {
  .analytics-overview {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .analytics-panels {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 600px) {
  .analytics-overview {
    gap: 8px;
  }

  .analytics-stat-card {
    padding: 12px;
  }

  .recent-review-row {
    gap: 8px;
  }

  .recent-review-time {
    display: none;
  }
}

/* --------------------------------------------------
   复习历史管理页面
-------------------------------------------------- */

.review-history-page {
  padding-bottom: 35px;
}

/* 导出按钮 */

.export-history-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 9px 15px;
  border: none;
  border-radius: 7px;
  color: white;
  background: #5369d9;
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}

.export-history-button:hover:not(:disabled) {
  background: #4055c4;
}

.export-history-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

/* 顶部统计 */

.review-history-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 18px;
}

.review-history-stat {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: white;
}

.review-history-stat span {
  color: #8490a3;
  font-size: 10px;
}

.review-history-stat strong {
  color: #354052;
  font-size: 23px;
}

/* 筛选面板 */

.review-history-filter-panel {
  margin-bottom: 16px;
  padding: 17px;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: white;
}

.review-history-filter-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 15px;
}

.review-history-filter-heading > div {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.review-history-filter-heading strong {
  color: #354052;
  font-size: 12px;
}

.review-history-filter-heading span {
  color: #9ba4b2;
  font-size: 9px;
}

.review-history-reset {
  padding: 6px 10px;
  border: 1px solid #e3e6ee;
  border-radius: 6px;
  color: #68758a;
  background: white;
  font-size: 10px;
}

.review-history-reset:hover:not(:disabled) {
  color: #5369d9;
  border-color: #cbd3ff;
  background: #f7f8ff;
}

.review-history-reset:disabled {
  opacity: 0.4;
}

.review-history-filters {
  display: grid;
  grid-template-columns: minmax(180px, 1.5fr) repeat(3, minmax(125px, 1fr));
  gap: 12px;
}

.review-history-filter-field {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.review-history-filter-field label {
  color: #7e899b;
  font-size: 10px;
}

.review-history-filter-field input,
.review-history-filter-field select {
  width: 100%;
  min-width: 0;
  height: 36px;
  padding: 0 9px;
  border: 1px solid #e3e6ee;
  border-radius: 6px;
  outline: none;
  color: #536075;
  background: white;
  font-size: 10px;
}

.review-history-filter-field input:focus,
.review-history-filter-field select:focus {
  border-color: #8494ed;
  box-shadow: 0 0 0 2px rgba(83, 105, 217, 0.08);
}

.review-history-search-box {
  height: 36px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 9px;
  border: 1px solid #e3e6ee;
  border-radius: 6px;
  color: #9ba4b2;
  background: white;
}

.review-history-search-box:focus-within {
  border-color: #8494ed;
  box-shadow: 0 0 0 2px rgba(83, 105, 217, 0.08);
}

.review-history-search-box input {
  width: 100%;
  height: 32px;
  padding: 0;
  border: none;
  outline: none;
  color: #536075;
  background: transparent;
  font-size: 10px;
}

.review-history-search-box input:focus {
  box-shadow: none;
}

.review-history-search-box button {
  display: grid;
  flex-shrink: 0;
  place-items: center;
  padding: 3px;
  border: none;
  color: #9ba4b2;
  background: transparent;
}

/* 历史记录列表 */

.review-history-list {
  overflow: hidden;
  border: 1px solid #e8eaf1;
  border-radius: 9px;
  background: white;
}

.review-history-row {
  min-height: 75px;
  display: flex;
  align-items: center;
  gap: 15px;
  padding: 12px 17px;
  border-bottom: 1px solid #f0f1f5;
}

.review-history-row:last-child {
  border-bottom: none;
}

.review-history-row-icon {
  width: 37px;
  height: 37px;
  display: grid;
  flex-shrink: 0;
  place-items: center;
  border-radius: 8px;
  color: #5369d9;
  background: #edf0ff;
}

.review-history-word {
  min-width: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 0;
  border: none;
  background: transparent;
  text-align: left;
}

.review-history-word strong {
  overflow: hidden;
  max-width: 100%;
  color: #354052;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.review-history-word:hover strong {
  color: #5369d9;
}

.review-history-word span {
  color: #9ba4b2;
  font-size: 9px;
}

.review-history-familiarity {
  min-width: 110px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
}

.review-history-familiarity > span:last-child {
  color: #8e98a8;
  font-size: 9px;
}

.review-history-level {
  padding: 4px 9px;
  border-radius: 5px;
  font-size: 10px;
  font-weight: 600;
}

.review-level-1 {
  color: #b4534b;
  background: #fff0ed;
}

.review-level-2 {
  color: #b87934;
  background: #fff4e5;
}

.review-level-3 {
  color: #8c7a36;
  background: #fff9df;
}

.review-level-4 {
  color: #5576b8;
  background: #edf3ff;
}

.review-level-5 {
  color: #21845c;
  background: #eaf8f0;
}

.review-history-date {
  min-width: 150px;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 6px;
}

.review-history-date span {
  color: #a0a8b5;
  font-size: 9px;
}

.review-history-date strong {
  color: #69758a;
  font-size: 10px;
  font-weight: 500;
}

.review-history-open {
  width: 30px;
  height: 30px;
  display: grid;
  flex-shrink: 0;
  place-items: center;
  border: none;
  border-radius: 6px;
  color: #9ba4b2;
  background: transparent;
}

.review-history-open:hover {
  color: #5369d9;
  background: #edf0ff;
}

.review-history-empty {
  margin-top: 15px;
}

.review-history-heading-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.review-history-link {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 5px 7px;
  border: none;
  border-radius: 5px;
  color: #5369d9;
  background: transparent;
  font-size: 10px;
  white-space: nowrap;
}

.review-history-link:hover {
  background: #edf0ff;
}

@media (max-width: 900px) {
  .review-history-summary {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .review-history-filters {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 600px) {
  .review-history-summary {
    gap: 8px;
  }

  .review-history-stat {
    padding: 12px;
  }

  .review-history-filters {
    grid-template-columns: 1fr;
  }

  .review-history-row {
    gap: 9px;
    padding: 11px;
  }

  .review-history-familiarity {
    min-width: 75px;
  }

  .review-history-date {
    min-width: 0;
  }

  .review-history-date strong {
    font-size: 9px;
  }

  .review-history-open {
    display: none;
  }
}

/* --------------------------------------------------
   学习记录批量管理
-------------------------------------------------- */

.learning-bulk-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 15px;
  margin: 12px 0;
  padding: 12px 15px;
  border: 1px solid #e8eaf1;
  border-radius: 8px;
  background: white;
}

.learning-select-all {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: #596579;
  font-size: 11px;
  cursor: pointer;
}

.learning-select-checkbox {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  accent-color: #5269db;
  cursor: pointer;
}

.learning-bulk-actions {
  display: flex;
  align-items: center;
  gap: 13px;
}

.learning-bulk-actions > span {
  color: #929bad;
  font-size: 11px;
}

.learning-bulk-actions .danger-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

/* --------------------------------------------------
   复习历史管理
-------------------------------------------------- */

.review-history-page-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

@media (max-width: 600px) {
  .learning-bulk-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .learning-bulk-actions {
    width: 100%;
    justify-content: space-between;
  }

  .review-history-page-actions {
    flex-wrap: wrap;
    justify-content: flex-end;
  }
}

/* --------------------------------------------------
   词典启用与停用管理
-------------------------------------------------- */

/* 词典状态标签 */
.dictionary-status-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 9px;
  border: 1px solid transparent;
  border-radius: 6px;
  font-size: 10px;
  font-weight: 600;
  white-space: nowrap;
}

.dictionary-status-dot {
  width: 6px;
  height: 6px;
  flex-shrink: 0;
  border-radius: 50%;
}

.status-enabled {
  color: #25845a;
  border-color: #d7eee1;
  background: #f0faf4;
}

.status-enabled .dictionary-status-dot {
  background: #35ad73;
  box-shadow: 0 0 0 3px rgba(53, 173, 115, 0.12);
}

.status-disabled {
  color: #8992a1;
  border-color: #e7e9ee;
  background: #f6f7f9;
}

.status-disabled .dictionary-status-dot {
  background: #a6adba;
}

/* 启用、停用按钮与切换、移除按钮统一基础样式 */
.dictionary-switch-button,
.dictionary-remove-button,
.dictionary-toggle-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 12px;
  border: 1px solid #e5e8ef;
  border-radius: 6px;
  background: white;
  font-size: 10px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.18s ease;
}

/* 停用按钮 */
.dictionary-toggle-button {
  color: #bd6670;
  border-color: #efd9dc;
}

.dictionary-toggle-button:hover {
  color: #a93e4b;
  border-color: #dfabb2;
  background: #fff6f6;
}

/* 启用按钮 */
.dictionary-enable-button {
  color: #287b55;
  border-color: #d1e9dc;
}

.dictionary-enable-button:hover {
  color: #206c49;
  border-color: #a9d8bd;
  background: #f1faf4;
}

/* 非当前词典图标 */
.dictionary-item:not(.dictionary-item-active) .dictionary-icon {
  opacity: 0.65;
}

.dictionary-item:not(.dictionary-item-active):has(.status-disabled) {
  background: #fafbfc;
}

@media (max-width: 650px) {
  .dictionary-item-actions {
    flex-wrap: wrap;
  }
}

.dictionary-enabled-badge {
  display: inline-flex;
  align-items: center;
  padding: 3px 8px;
  border-radius: 20px;
  color: #287b55;
  background: #eaf7ef;
  font-size: 10px;
  font-weight: 600;
}

.dictionary-disabled-badge {
  color: #9299a6;
  background: #f0f1f4;
}

.dictionary-toggle-button {
  padding: 7px 12px;
  border: 1px solid #e5c8c5;
  border-radius: 6px;
  color: #bd5148;
  background: white;
  font-size: 11px;
  cursor: pointer;
}

.dictionary-toggle-button:hover {
  background: #fff5f4;
}

.dictionary-enable-button {
  border-color: #c7e6d3;
  color: #287b55;
}

.dictionary-enable-button:hover {
  background: #effaf3;
}

.dictionary-item:not(.dictionary-item-active) .dictionary-icon {
  opacity: 0.65;
}

.dictionary-item:not(.dictionary-item-active):has(.dictionary-disabled-badge) {
  background: #fafbfc;
}

@media (max-width: 650px) {
  .dictionary-item-actions {
    flex-wrap: wrap;
  }
}

/* --------------------------------------------------
   左侧导航栏收缩状态
-------------------------------------------------- */

.sidebar-collapsed {
  width: 76px;
  min-width: 76px;
  padding-right: 10px;
  padding-left: 10px;
}

.sidebar-collapsed .sidebar-brand {
  justify-content: center;
  padding: 0;
  margin-bottom: 35px;
}

.sidebar-collapsed .brand-name,
.sidebar-collapsed .sidebar-label,
.sidebar-collapsed .nav-item span,
.sidebar-collapsed .dictionary-status-text,
.sidebar-collapsed .status-dot,
.sidebar-collapsed .sidebar-version {
  display: none;
}

.sidebar-collapsed .nav-item {
  justify-content: center;
  gap: 0;
  padding: 0;
}

.sidebar-collapsed .sidebar-section {
  margin-bottom: 25px;
}

.sidebar-collapsed .sidebar-dictionary {
  justify-content: center;
  padding: 10px 0;
}

/* 收缩按钮位于左下方 */
.sidebar-collapsed .sidebar-toggle-container {
  justify-content: center;
  padding: 10px 0 12px;
}

.sidebar-collapsed .sidebar-toggle {
  width: 42px;
  justify-content: center;
  padding: 0;
}

/* --------------------------------------------------
   左侧导航栏固定布局
-------------------------------------------------- */

/* 可滚动的导航内容 */
.sidebar-navigation {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-width: none;
}

.sidebar-navigation::-webkit-scrollbar {
  display: none;
}

/* 底部收缩按钮 */
.sidebar-toggle-container {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-start;
  padding: 10px 10px 12px;
}

/* 收缩按钮 */
.sidebar-toggle {
  width: 100%;
  height: 38px;

  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 10px;

  padding: 0 11px;

  border: 1px solid #344057;
  border-radius: 7px;

  color: #aeb8ca;
  background: transparent;

  font-size: 11px;
  cursor: pointer;

  transition: 0.18s ease;
}

.sidebar-toggle:hover {
  color: white;
  background: #34436a;
  border-color: #52638a;
}

/* 底部词典信息固定，不参与导航内容滚动 */
.sidebar-bottom {
  flex-shrink: 0;
  margin-top: 0;
}
/* --------------------------------------------------
   自定义窗口标题栏和窗口控制按钮
-------------------------------------------------- */

.topbar {
  height: 52px;
  min-height: 52px;
  flex-shrink: 0;

  display: flex;
  align-items: center;
  justify-content: space-between;

  padding: 0 12px 0 35px;

  border-bottom: 1px solid #e9ebf1;
  background: #fff;

  user-select: none;
}

/* 右侧窗口控制区域 */
.window-controls {
  height: 100%;
  display: flex;
  align-items: center;
  flex-shrink: 0;
  gap: 7px;
}

/* 窗口控制按钮 */
.window-control-button {
  width: 36px;
  height: 34px;

  display: grid;
  place-items: center;

  padding: 0;
  border: none;
  border-radius: 6px;

  color: #687386;
  background: transparent;

  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}

.window-control-button:hover {
  color: #344054;
  background: #f0f2f6;
}

/* 设置按钮 */
.settings-control {
  margin-right: 5px;
  color: #596a91;
}

.settings-control:hover {
  color: #5269db;
  background: #edf0ff;
}

/* 三个竖排圆点分隔 */
.window-control-separator {
  width: 20px;
  height: 34px;

  display: grid;
  place-items: center;

  margin: 0 5px;

  color: #a1a9b6;
}

/* 关闭按钮 */
.close-control:hover {
  color: #fff;
  background: #e5484d;
}

/* 鼠标拖动区域 */
.topbar,
.topbar .breadcrumb {
  cursor: default;
}
</style>
