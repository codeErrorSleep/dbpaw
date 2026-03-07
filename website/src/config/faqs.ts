export const FAQS = {
  en: [
    {
      question: 'Which databases are currently supported?',
      answer:
        'DbPaw currently supports PostgreSQL, MySQL, MariaDB (MySQL-compatible), TiDB (MySQL-compatible), SQLite, SQL Server, and ClickHouse (Preview, Read-only), with more drivers planned in future updates.'
    },
    {
      question: 'Does DbPaw work with remote databases?',
      answer:
        'Yes. DbPaw supports SSH tunneling so you can connect securely to remote database instances.'
    },
    {
      question: 'How does AI assistance work?',
      answer:
        'You can use the AI sidebar to generate SQL, explain queries, and optimize statements based on your intent.'
    },
    {
      question: 'Is DbPaw open source?',
      answer: 'Yes. DbPaw is released under the MIT License on GitHub.'
    },
    {
      question: 'I see a macOS security warning. What should I do?',
      answer:
        'Move DbPaw.app to /Applications and remove quarantine attributes using the command shown on the download page.'
    }
  ],
  zh: [
    {
      question: '当前支持哪些数据库？',
      answer:
        'DbPaw 目前支持 PostgreSQL、MySQL、MariaDB（兼容 MySQL）、TiDB（兼容 MySQL）、SQLite、SQL Server 和 ClickHouse（预览版，只读），更多驱动正在计划中。'
    },
    {
      question: '支持远程数据库连接吗？',
      answer:
        '支持。DbPaw 内置 SSH 隧道支持，可以安全地连接到远程数据库实例。'
    },
    {
      question: 'AI 辅助功能是如何工作的？',
      answer:
        '您可以使用 AI 侧边栏来生成 SQL、解释查询逻辑，并根据您的意图优化语句。'
    },
    {
      question: 'DbPaw 是开源的吗？',
      answer: '是的。DbPaw 基于 MIT 许可证在 GitHub 上开源。'
    },
    {
      question: '我在 macOS 上看到了安全警告，该怎么办？',
      answer:
        '请将 DbPaw.app 移动到 /Applications 目录，并使用下载页面上显示的命令移除隔离属性。'
    }
  ]
} as const;
