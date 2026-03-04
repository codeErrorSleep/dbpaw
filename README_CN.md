# DbPaw

![DbPaw Logo](public/product-icon.png)

[English](README.md) | 简体中文 | [日本語](README_JA.md)

> **一个聚焦高效查询与数据探索的现代数据库客户端，并提供可选 AI 助手。**

[![Release](https://img.shields.io/github/v/release/codeErrorSleep/dbpaw?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg?style=flat-square)](https://tauri.app)

**DbPaw** 帮助你连接 PostgreSQL、MySQL、SQLite、SQL Server（MSSQL）与 ClickHouse（预览版），高效编写和执行 SQL，并在清爽的桌面 UI 中查看与探索数据。

## ✅ 你可以用它做什么

- 连接 PostgreSQL、MySQL、SQLite、SQL Server（MSSQL）与 ClickHouse（预览版，当前只读）
- 编写与执行 SQL：语法高亮、自动补全、一键格式化
- 在数据网格中浏览结果，支持过滤、排序、分页与导出
- 使用 Saved Queries 保存并复用常用 SQL 脚本
- 使用 AI 侧边栏辅助写 SQL、解释查询（可选）
- 通过 SSH 隧道访问远程数据库

## 🖼️ 界面预览

![DbPaw 主工作区](docs/screenshots/01-overview.png.png)

![DbPaw 主工作区（深色模式）](docs/screenshots/01-overview-black.png)


| 连接管理 | SQL 编辑器 |
| --- | --- |
| ![连接管理](docs/screenshots/02-connect.png) | ![SQL 编辑器](docs/screenshots/03-editor.png) |

| 数据网格 | AI 助手 |
| --- | --- |
| ![数据网格](docs/screenshots/04-ddl-grid.png) | ![AI 助手](docs/screenshots/05-ai.png) |

## ✨ 特性

- **多数据库支持**：PostgreSQL、MySQL、SQLite、SQL Server（MSSQL）、ClickHouse（预览版，只读）。
- **AI 智能辅助（可选）**：辅助编写 SQL、解释查询逻辑。
- **安全连接**：支持 SSH 隧道，安全访问远程数据库。
- **SQL 编辑器**：语法高亮、自动补全、格式化、Saved Queries。
- **数据网格**：支持过滤、排序、分页与结果导出。
- **现代桌面 UI**：React + TailwindCSS + Shadcn/UI，内置深色模式。
- **高性能运行时**：基于 Tauri（Rust 后端 + Web 前端）。

## 📥 安装

前往 [Releases](https://github.com/codeErrorSleep/dbpaw/releases) 页面下载适合您操作系统的最新版本。

### macOS 用户

1. 从 [Releases](https://github.com/codeErrorSleep/dbpaw/releases) 下载 macOS 版本的 `DbPaw`。
2. 将 `DbPaw.app` 移动到 `/Applications` 文件夹。
3. 打开应用。

如果 macOS 提示“无法识别的开发者”并阻止打开：

1. 打开 **系统设置** → **隐私与安全性**。
2. 滚动到下方 **安全性** 区域，找到关于 `DbPaw` 被阻止的提示。
3. 点击 **仍要打开**，并在弹窗中确认 **打开**。

如果在打开应用时遇到“DbPaw 已损坏”（Gatekeeper 隔离标记）：

1. 将 `DbPaw.app` 移动到 `/Applications` 文件夹。
2. 打开**终端**并运行以下命令：
   ```bash
   sudo xattr -d com.apple.quarantine /Applications/DbPaw.app
   ```
3. 现在可以正常打开应用。

_注意：这是因为应用尚未经过 Apple 公证。_

### Windows 用户

1. 从 [Releases](https://github.com/codeErrorSleep/dbpaw/releases) 下载 Windows 安装包或可执行文件。
2. 运行安装程序/可执行文件。

如果 Windows 弹出“Windows 已保护你的电脑”（SmartScreen）等安全警告：

1. 点击 **更多信息**。
2. 点击 **仍要运行**。

若设备由组织统一管理，可能需要管理员允许该应用运行。

## 🔐 安全与隐私

- DbPaw 是本地桌面应用：数据库连接从你的设备直接访问数据库。
- AI 功能为可选：启用后会向你配置的 AI 服务商发送你的输入、最近对话上下文，以及（可选）schema 概览（表/列/类型等元信息）。
- AI 对话会保存在本地；AI 服务商的 API Key 会在本地加密存储。
- 桌面端未内置遥测/分析 SDK。

## 🛠️ 开发

如果您想贡献代码或从源码构建，请按照以下步骤操作：

### 前提条件

- [Rust](https://www.rust-lang.org/tools/install) (最新稳定版)
- [Bun](https://bun.sh/) 或 Node.js (v18+)

### 设置

1. **克隆仓库**

   ```bash
   git clone https://github.com/codeErrorSleep/dbpaw.git
   cd dbpaw
   ```

2. **安装前端依赖**

   ```bash
   bun install
   ```

3. **运行开发模式**

   **仅前端（模拟模式）** - 推荐用于 UI 开发：

   ```bash
   bun dev:mock
   ```

   **完整应用（Tauri + Rust）** - 用于完整功能测试：

   ```bash
   bun tauri dev
   ```

4. **构建生产版本**
   ```bash
   bun tauri build
   ```

## 🏗️ 技术栈

- **核心**: [Tauri v2](https://v2.tauri.app/) (Rust)
- **前端**: [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/)
- **样式**: [TailwindCSS v4](https://tailwindcss.com/), [Shadcn/UI](https://ui.shadcn.com/)
- **状态管理**: React Hooks & Context
- **编辑器**: [Monaco Editor](https://microsoft.github.io/monaco-editor/) / CodeMirror

## 🌐 官网

- 官方官网位于 `website/` 目录，基于 [Astro](https://astro.build/) 构建。
- 本地开发：
  ```bash
  bun run website:dev
  ```
- 生产构建：
  ```bash
  bun run website:build
  ```

### 版本同步机制

- 官网会从以下地址拉取最新版本：
  `https://api.github.com/repos/codeErrorSleep/dbpaw/releases/latest`
- 官网展示的版本号与下载链接由 GitHub Releases 资产自动生成。
- 若构建时 GitHub API 不可用，官网会自动回退到 `website/src/config/fallback.ts`，确保构建不中断。

## 📄 许可证

本项目采用 MIT 许可证 - 详情请参阅 [LICENSE](LICENSE) 文件。

## ❤️ 致谢

感谢你试用 DbPaw。若它对你有帮助，欢迎给本仓库点个 Star 支持一下，这对项目发展很重要！
