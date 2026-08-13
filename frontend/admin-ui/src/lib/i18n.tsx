import React, { createContext, useContext, useState } from 'react'

export type Language = 'zh' | 'en'

export interface I18nDictionary {
  [key: string]: string | I18nDictionary
}

export const zhDict: Record<string, string> = {
  // 通用
  'app.name': 'HertaBase 控制台',
  'app.version': '版本',
  'app.tagline': '新一代单二进制后端即服务 (BaaS)',
  'app.loading': '加载中...',
  'app.save': '保存',
  'app.cancel': '取消',
  'app.confirm': '确认',
  'app.delete': '删除',
  'app.edit': '编辑',
  'app.create': '新建',
  'app.search': '搜索',
  'app.actions': '操作',
  'app.status': '状态',
  'app.refresh': '刷新',
  'app.close': '关闭',
  'app.copy': '复制',
  'app.copied': '已复制到剪贴板',
  'app.all': '全部',
  'app.theme.dark': '深色模式',
  'app.theme.light': '浅色模式',
  'app.lang.switch': '语言',

  // 认证 / 登录
  'auth.login.title': 'HertaBase 管理员登录',
  'auth.login.subtitle': '请输入超级管理员账号凭据以访问控制台',
  'auth.email': '管理员邮箱',
  'auth.password': '登录密码',
  'auth.email.placeholder': 'admin@example.com',
  'auth.password.placeholder': '••••••••',
  'auth.login.btn': '登 录',
  'auth.logging_in': '正在验证...',
  'auth.login.success': '登录成功，欢迎回来！',
  'auth.login.failed': '认证失败，请检查邮箱或密码',
  'auth.logout': '退出登录',
  'auth.logout.confirm': '确定要退出当前管理员账号吗？',
  'auth.remember_me': '保持登录状态',
  'auth.current_admin': '当前管理员',

  // 侧边栏与导航
  'nav.workspace': '工作空间',
  'nav.database': '数据库',
  'nav.collections': '集合管理',
  'nav.base_collections': '基础数据表',
  'nav.auth_collections': '用户认证表',
  'nav.system_collections': '系统内核表',
  'nav.logs': '系统日志',
  'nav.settings': '系统设置',
  'nav.sql_console': 'SQL 查询终端',
  'nav.swagger': 'API 文档',
  'nav.new_collection': '新建集合',

  // 集合管理
  'collections.title': '集合结构管理',
  'collections.subtitle': '配置动态数据表 Schema 约束与细粒度 API Rules 规则',
  'collections.create': '创建新集合',
  'collections.name': '集合名称',
  'collections.name_help': '仅限英文字母、数字和下划线，例如 posts、comments',
  'collections.type': '集合类型',
  'collections.type.base': '普通数据表',
  'collections.type.auth': '用户认证表',
  'collections.fields': '字段定义',
  'collections.rules': '访问权限规则',
  'collections.rule.list': '列表规则',
  'collections.rule.view': '查看规则',
  'collections.rule.create': '创建规则',
  'collections.rule.update': '更新规则',
  'collections.rule.delete': '删除规则',
  'collections.rule.help': '填写 SurrealQL 布尔表达式，留空或 null 表示仅管理员可访问，@request.auth.id != null 表示需登录',
  'collections.add_field': '添加字段',
  'collections.field_name': '字段名',
  'collections.field_type': '字段类型',
  'collections.field_required': '必填',
  'collections.field_unique': '唯一',
  'collections.empty': '暂无自定义集合，点击右上方按钮开始创建。',
  'collections.created_success': '集合创建成功',
  'collections.updated_success': '集合更新成功',
  'collections.deleted_success': '集合已删除',
  'collections.delete_confirm': '确定要删除集合 {name} 吗？此操作将同时清空该表所有数据且不可恢复！',

  // 记录管理
  'records.title': '数据记录',
  'records.new': '新建记录',
  'records.edit': '编辑记录',
  'records.delete_confirm': '确定要删除这条记录吗？',
  'records.deleted_success': '记录已删除',
  'records.saved_success': '记录保存成功',
  'records.filter_placeholder': '过滤条件 (如: status = "published" AND score > 10)',
  'records.sort_placeholder': '排序 (如: -created_at, title)',
  'records.expand_placeholder': '关联展开 (如: author, comments.user)',
  'records.live_active': 'SSE 实时推力已激活',
  'records.live_connecting': 'SSE 正在连接...',
  'records.live_offline': 'SSE 离线',
  'records.total_items': '共 {total} 条记录',
  'records.page_size': '每页条数',
  'records.empty': '当前集合暂无记录',
  'records.json_view': 'JSON 原始数据',

  // 日志页面
  'logs.title': '系统运行与 Hook 日志',
  'logs.subtitle': '实时查看后端微内核、JS VM 沙盒及 REST 接口调用日志',
  'logs.clear': '清空日志',
  'logs.auto_scroll': '自动滚动',
  'logs.search_placeholder': '搜索日志消息或关键词...',
  'logs.empty': '暂无日志输出',
  'logs.level.all': '全部级别',
  'logs.export': '导出日志',

  // 设置页面
  'settings.title': '系统设置中心',
  'settings.subtitle': '管理应用核心参数、定时任务调度、SQL 查询与数据迁移',
  'settings.tab.app': '应用参数配置',
  'settings.tab.cron': '定时任务调度',
  'settings.tab.sql': 'SQL 查询终端',
  'settings.tab.migration': '集合导入与导出',

  // 应用配置
  'settings.app.server_host': '监听主机',
  'settings.app.server_port': '服务端口',
  'settings.app.data_dir': '数据持久化目录',
  'settings.app.hooks_dir': 'JS 扩展脚本目录',
  'settings.app.db_engine': '数据库引擎',
  'settings.app.log_level': '日志输出级别',
  'settings.app.access_ttl': 'Access Token 有效期 (秒)',
  'settings.app.refresh_ttl': 'Refresh Token 有效期 (秒)',
  'settings.app.save_btn': '保存配置',
  'settings.app.saved': '系统参数配置已保存',

  // Cron 任务
  'settings.cron.title': 'JS VM & 系统定时任务',
  'settings.cron.name': '任务标识',
  'settings.cron.expr': 'Cron 表达式 (6段)',
  'settings.cron.next_run': '下次预计触发',
  'settings.cron.status': '运行状态',
  'settings.cron.last_exec': '最近执行耗时',
  'settings.cron.empty': '当前未注册任何定时任务。在 hb_hooks 脚本中使用 cronAdd() 即可自动注册。',

  // SQL Console
  'settings.sql.title': 'SurrealQL 交互式执行终端',
  'settings.sql.desc': '支持输入任意 SurrealQL 语句（SELECT, CREATE, UPDATE, RELATE, DEFINE...）直接操作数据库',
  'settings.sql.execute': '执行查询 (Ctrl+Enter)',
  'settings.sql.executing': '正在执行...',
  'settings.sql.time_cost': '耗时: {time}ms',
  'settings.sql.rows_affected': '返回/影响行数: {rows}',
  'settings.sql.tab_json': 'JSON 响应',
  'settings.sql.tab_table': '数据表格',
  'settings.sql.history': '查询历史',

  // 导入导出
  'settings.migration.export_title': '备份与导出数据',
  'settings.migration.export_desc': '将选定集合的 Schema 结构定义与完整数据集导出为 JSON 文件',
  'settings.migration.export_btn': '立即导出备份 (.json)',
  'settings.migration.import_title': '导入与恢复数据',
  'settings.migration.import_desc': '上传之前导出的 JSON 备份文件以快速重建集合结构与填充数据',
  'settings.migration.import_btn': '选择文件并开始导入',
  'settings.migration.import_success': '数据导入与同步成功！',
  'settings.migration.select_all': '全选集合',

  // 状态栏
  'status.db': '数据库',
  'status.engine': '引擎',
  'status.ready': '就绪',
  'status.realtime': '实时总线',
}

export const enDict: Record<string, string> = {
  // Common
  'app.name': 'HertaBase Console',
  'app.version': 'Version',
  'app.tagline': 'Next-gen Single-binary Backend-as-a-Service',
  'app.loading': 'Loading...',
  'app.save': 'Save',
  'app.cancel': 'Cancel',
  'app.confirm': 'Confirm',
  'app.delete': 'Delete',
  'app.edit': 'Edit',
  'app.create': 'Create',
  'app.search': 'Search',
  'app.actions': 'Actions',
  'app.status': 'Status',
  'app.refresh': 'Refresh',
  'app.close': 'Close',
  'app.copy': 'Copy',
  'app.copied': 'Copied to clipboard',
  'app.all': 'All',
  'app.theme.dark': 'Dark Theme',
  'app.theme.light': 'Light Theme',
  'app.lang.switch': 'Language',

  // Auth / Login
  'auth.login.title': 'HertaBase Admin Login',
  'auth.login.subtitle': 'Enter superuser credentials to access the console',
  'auth.email': 'Admin Email',
  'auth.password': 'Password',
  'auth.email.placeholder': 'admin@example.com',
  'auth.password.placeholder': '••••••••',
  'auth.login.btn': 'Sign In',
  'auth.logging_in': 'Authenticating...',
  'auth.login.success': 'Signed in successfully. Welcome back!',
  'auth.login.failed': 'Authentication failed. Please check your credentials.',
  'auth.logout': 'Sign Out',
  'auth.logout.confirm': 'Are you sure you want to sign out?',
  'auth.remember_me': 'Remember Me',
  'auth.current_admin': 'Admin Account',

  // Navigation
  'nav.workspace': 'Workspace',
  'nav.database': 'Database',
  'nav.collections': 'Collections',
  'nav.base_collections': 'Base Collections',
  'nav.auth_collections': 'Auth Collections',
  'nav.system_collections': 'System Collections',
  'nav.logs': 'Logs',
  'nav.settings': 'Settings',
  'nav.sql_console': 'SQL Console',
  'nav.swagger': 'OpenAPI Docs',
  'nav.new_collection': 'New Collection',

  // Collections
  'collections.title': 'Collection Schema Management',
  'collections.subtitle': 'Manage dynamic table schemas, validation constraints, and API rules',
  'collections.create': 'Create Collection',
  'collections.name': 'Collection Name',
  'collections.name_help': 'Alphanumeric and underscores only, e.g., posts, comments',
  'collections.type': 'Collection Type',
  'collections.type.base': 'Base Collection',
  'collections.type.auth': 'Auth Collection',
  'collections.fields': 'Schema Fields',
  'collections.rules': 'Access Rules',
  'collections.rule.list': 'List Rule',
  'collections.rule.view': 'View Rule',
  'collections.rule.create': 'Create Rule',
  'collections.rule.update': 'Update Rule',
  'collections.rule.delete': 'Delete Rule',
  'collections.rule.help': 'SurrealQL boolean expression. Leave empty/null for admin-only access, @request.auth.id != null for authenticated users.',
  'collections.add_field': 'Add Field',
  'collections.field_name': 'Field Name',
  'collections.field_type': 'Field Type',
  'collections.field_required': 'Required',
  'collections.field_unique': 'Unique',
  'collections.empty': 'No custom collections yet. Click the top-right button to create one.',
  'collections.created_success': 'Collection created successfully',
  'collections.updated_success': 'Collection updated successfully',
  'collections.deleted_success': 'Collection deleted',
  'collections.delete_confirm': 'Are you sure you want to delete collection {name}? All record data will be permanently purged!',

  // Records
  'records.title': 'Records',
  'records.new': 'New Record',
  'records.edit': 'Edit Record',
  'records.delete_confirm': 'Are you sure you want to delete this record?',
  'records.deleted_success': 'Record deleted',
  'records.saved_success': 'Record saved successfully',
  'records.filter_placeholder': 'Filter expression (e.g. status = "published" AND score > 10)',
  'records.sort_placeholder': 'Sort by (e.g. -created_at, title)',
  'records.expand_placeholder': 'Expand relations (e.g. author, comments.user)',
  'records.live_active': 'SSE Realtime Connected',
  'records.live_connecting': 'SSE Connecting...',
  'records.live_offline': 'SSE Offline',
  'records.total_items': '{total} items in total',
  'records.page_size': 'Page Size',
  'records.empty': 'No records in this collection',
  'records.json_view': 'Raw JSON',

  // Logs
  'logs.title': 'System & Hook Runtime Logs',
  'logs.subtitle': 'Live console stream from backend microkernel, JS runtime sandbox and API requests',
  'logs.clear': 'Clear Console',
  'logs.auto_scroll': 'Auto Scroll',
  'logs.search_placeholder': 'Filter messages or keywords...',
  'logs.empty': 'No logs available',
  'logs.level.all': 'All Levels',
  'logs.export': 'Export Logs',

  // Settings
  'settings.title': 'System Settings',
  'settings.subtitle': 'Manage application parameters, cron schedules, SQL queries and data migration',
  'settings.tab.app': 'App Settings',
  'settings.tab.cron': 'Scheduled Tasks',
  'settings.tab.sql': 'SQL Console',
  'settings.tab.migration': 'Import & Export',

  // App Settings
  'settings.app.server_host': 'Server Host',
  'settings.app.server_port': 'Server Port',
  'settings.app.data_dir': 'Data Directory',
  'settings.app.hooks_dir': 'Hooks Directory',
  'settings.app.db_engine': 'Database Engine',
  'settings.app.log_level': 'Log Level',
  'settings.app.access_ttl': 'Access Token TTL (sec)',
  'settings.app.refresh_ttl': 'Refresh Token TTL (sec)',
  'settings.app.save_btn': 'Save Configuration',
  'settings.app.saved': 'System configuration saved',

  // Cron
  'settings.cron.title': 'JS VM & System Scheduled Jobs',
  'settings.cron.name': 'Job Name',
  'settings.cron.expr': 'Cron Expression (6-part)',
  'settings.cron.next_run': 'Next Execution',
  'settings.cron.status': 'Status',
  'settings.cron.last_exec': 'Last Duration',
  'settings.cron.empty': 'No scheduled jobs registered. Use cronAdd() in hb_hooks to register jobs automatically.',

  // SQL Console
  'settings.sql.title': 'SurrealQL Interactive Query Console',
  'settings.sql.desc': 'Execute any SurrealQL statements (SELECT, CREATE, UPDATE, RELATE, DEFINE...) directly against the database',
  'settings.sql.execute': 'Run Query (Ctrl+Enter)',
  'settings.sql.executing': 'Executing...',
  'settings.sql.time_cost': 'Latency: {time}ms',
  'settings.sql.rows_affected': 'Returned/Affected: {rows}',
  'settings.sql.tab_json': 'JSON Response',
  'settings.sql.tab_table': 'Data Table',
  'settings.sql.history': 'Query History',

  // Import / Export
  'settings.migration.export_title': 'Backup & Export',
  'settings.migration.export_desc': 'Export selected collection schema definitions and complete record datasets as JSON backup',
  'settings.migration.export_btn': 'Export Backup File (.json)',
  'settings.migration.import_title': 'Restore & Import',
  'settings.migration.import_desc': 'Upload a previously exported JSON backup file to reconstruct collection schemas and restore data',
  'settings.migration.import_btn': 'Select File & Start Import',
  'settings.migration.import_success': 'Data imported and synced successfully!',
  'settings.migration.select_all': 'Select All Collections',

  // Status Bar
  'status.db': 'Database',
  'status.engine': 'Engine',
  'status.ready': 'Ready',
  'status.realtime': 'Realtime Bus',
}

interface I18nContextType {
  lang: Language
  setLang: (lang: Language) => void
  t: (key: string, params?: Record<string, string | number>) => string
}

const I18nContext = createContext<I18nContextType>({
  lang: 'zh',
  setLang: () => {},
  t: (key) => key,
})

const LANG_STORAGE_KEY = 'hb_admin_lang'

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLangState] = useState<Language>(() => {
    const saved = localStorage.getItem(LANG_STORAGE_KEY)
    if (saved === 'en' || saved === 'zh') return saved
    return 'zh' // 默认中文
  })

  const setLang = (newLang: Language) => {
    setLangState(newLang)
    localStorage.setItem(LANG_STORAGE_KEY, newLang)
  }

  const t = (key: string, params?: Record<string, string | number>): string => {
    const dict = lang === 'zh' ? zhDict : enDict
    let text = dict[key] || zhDict[key] || key
    if (params) {
      Object.entries(params).forEach(([k, v]) => {
        text = text.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v))
      })
    }
    return text
  }

  return (
    <I18nContext.Provider value={{ lang, setLang, t }}>
      {children}
    </I18nContext.Provider>
  )
}

export function useI18n() {
  return useContext(I18nContext)
}
