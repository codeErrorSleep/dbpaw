export const FEATURES = {
  en: [
    {
      title: 'Tiny footprint',
      summary:
        'Installer ≈10 MB and ~80 MB on disk. Built with Rust + Tauri for fast startup and ultra‑low idle memory.',
      tag: 'Lightweight'
    },
    {
      title: 'Truly modern',
      summary:
        'No cockpit‑style complexity. We cut the 99% you never use and focus on everyday workflows for smoother, more intuitive operations.',
      tag: 'Modern'
    },
    {
      title: 'Cross‑platform',
      summary: 'Runs on macOS, Windows, and Linux — use the same tool everywhere.',
      tag: 'Cross‑platform'
    },
    {
      title: 'Database compatibility',
      summary:
        'Supports MySQL, MariaDB (MySQL-compatible), PostgreSQL, ClickHouse, TiDB, SQL Server, and SQLite (actively expanding).',
      tag: 'DBs'
    },
    {
      title: 'Looks great',
      summary:
        'Rich theme system with dark/light and high/low saturation styles for all‑day comfort.',
      tag: 'Themes'
    },
    {
      title: 'AI assistance (experimental)',
      summary:
        'Summarize SQL, explain schemas, and analyze slow queries. Security is under active refinement; local and optional cloud modes planned.',
      tag: 'AI'
    },
    {
      title: 'Completely free',
      summary:
        'No login, no payments, no memberships, no ads. Download and go.',
      tag: 'Free'
    }
  ],
  zh: [
    {
      title: '极致轻量',
      summary:
        '安装包 ≈10MB，磁盘占用 ≈80MB。基于 Rust + Tauri 构建，启动极快，内存占用极低。',
      tag: '轻量'
    },
    {
      title: '真正现代化',
      summary:
        '告别复杂的“驾驶舱”界面。我们精简了 99% 不常用的功能，专注于让日常操作更直观、丝滑。',
      tag: '现代'
    },
    {
      title: '跨平台',
      summary: '支持 macOS、Windows 和 Linux —— 无论在哪里都使用同一套工具。',
      tag: '跨平台'
    },
    {
      title: '数据库兼容',
      summary:
        '支持 MySQL、MariaDB (兼容 MySQL)、PostgreSQL、ClickHouse、TiDB、SQL Server 和 SQLite（持续扩展中）。',
      tag: '多数据库'
    },
    {
      title: '颜值在线',
      summary:
        '提供丰富的深色/浅色主题，以及多种高/低饱和度风格，全天候舒适体验。',
      tag: '主题'
    },
    {
      title: 'AI 智能辅助 (实验性)',
      summary:
        '支持 SQL 归纳、表结构解释和慢查询分析。安全性持续优化中；规划支持本地/可选云端模式。',
      tag: 'AI'
    },
    {
      title: '完全免费',
      summary:
        '无登录、无付费、无会员、无广告。下载即用。',
      tag: '免费'
    }
  ]
} as const;
