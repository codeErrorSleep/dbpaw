# DbPaw

![DbPaw Logo](public/product-icon.png)

English | [简体中文](README_CN.md) | [日本語](README_JA.md)

> **A modern database client focused on efficient querying and exploration, with optional AI assistance.**

[![Release](https://img.shields.io/github/v/release/codeErrorSleep/dbpaw?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg?style=flat-square)](https://tauri.app)

**DbPaw** helps you connect to PostgreSQL, MySQL, SQLite, SQL Server (MSSQL), and ClickHouse (preview), write and run SQL efficiently, and inspect data in a clean desktop UI.

## ✅ What You Can Do

- Connect to PostgreSQL, MySQL, SQLite, SQL Server (MSSQL), and ClickHouse (preview, currently read-only)
- Write and run SQL with syntax highlighting, auto-completion, and one-click formatting
- Browse query results in a data grid with filtering, sorting, pagination, and export
- Save and reuse frequently used SQL scripts with Saved Queries
- Use the AI sidebar to draft SQL and explain queries (optional)
- Access remote databases through SSH tunneling

## 🖼️ Screenshots

![DbPaw Main Workspace](docs/screenshots/01-overview.png.png)

![DbPaw Main Workspace (Dark)](docs/screenshots/01-overview-black.png)


| Connection | SQL Editor |
| --- | --- |
| ![Connection](docs/screenshots/02-connect.png) | ![Editor](docs/screenshots/03-editor.png) |

| Data Grid | AI Assistant |
| --- | --- |
| ![Grid](docs/screenshots/04-ddl-grid.png) | ![AI](docs/screenshots/05-ai.png) |

## ✨ Features

- **Multi-database support**: PostgreSQL, MySQL, SQLite, SQL Server (MSSQL), ClickHouse (preview, read-only).
- **AI assistance (optional)**: draft SQL and explain query logic.
- **Secure connectivity**: SSH tunneling support.
- **SQL editor**: syntax highlighting, auto-completion, formatting, and Saved Queries.
- **Data grid**: filtering, sorting, pagination, and export for result sets.
- **Modern desktop UI**: React + TailwindCSS + Shadcn/UI, with built-in dark mode.
- **Fast runtime**: powered by Tauri (Rust backend + web frontend).

## 📥 Installation

Go to the [Releases](https://github.com/codeErrorSleep/dbpaw/releases) page to download the latest version for your operating system.

### macOS Users

1. Download `DbPaw` for macOS from [Releases](https://github.com/codeErrorSleep/dbpaw/releases).
2. Move `DbPaw.app` to your `/Applications` folder.
3. Open the app.

If macOS blocks the app with an "Unidentified Developer" warning:

1. Open **System Settings** → **Privacy & Security**.
2. Scroll to the **Security** section and find the message about `DbPaw` being blocked.
3. Click **Open Anyway**, then confirm **Open**.

If you encounter a "DbPaw is damaged" warning (Gatekeeper quarantine):

1. Move `DbPaw.app` to your `/Applications` folder.
2. Open **Terminal** and run the following command:
   ```bash
   sudo xattr -d com.apple.quarantine /Applications/DbPaw.app
   ```
3. You can now open the app normally.

_Note: This is required because the app is not yet notarized by Apple._

### Windows Users

1. Download the installer or portable build from [Releases](https://github.com/codeErrorSleep/dbpaw/releases).
2. Run the installer / executable.

If Windows shows a security warning such as "Windows protected your PC" (SmartScreen):

1. Click **More info**.
2. Click **Run anyway**.

If your device is managed by an organization, you may need your IT admin to allow the app.

## 🔐 Security & Privacy

- DbPaw is a local desktop app. Your database connections run from your machine to your database.
- AI features are optional. When enabled, DbPaw sends your prompt, recent chat context, and (optionally) a schema overview (tables/columns/types) to the AI provider you configured.
- AI conversations are stored locally. AI provider API keys are stored encrypted on disk.
- No built-in telemetry/analytics SDK is included in the desktop app.

## 🛠️ Development

If you want to contribute or build from source, follow these steps:

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Bun](https://bun.sh/) or Node.js (v18+)

### Setup

1. **Clone the repository**

   ```bash
   git clone https://github.com/codeErrorSleep/dbpaw.git
   cd dbpaw
   ```

2. **Install frontend dependencies**

   ```bash
   bun install
   ```

3. **Run in Development Mode**

   **Frontend-only (Mock Mode)** - Recommended for UI work:

   ```bash
   bun dev:mock
   ```

   **Full App (Tauri + Rust)** - For full functionality testing:

   ```bash
   bun tauri dev
   ```

4. **Build for Production**
   ```bash
   bun tauri build
   ```

## 🏗️ Tech Stack

- **Core**: [Tauri v2](https://v2.tauri.app/) (Rust)
- **Frontend**: [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/)
- **Styling**: [TailwindCSS v4](https://tailwindcss.com/), [Shadcn/UI](https://ui.shadcn.com/)
- **State Management**: React Hooks & Context
- **Editor**: [Monaco Editor](https://microsoft.github.io/monaco-editor/) / CodeMirror

## 🌐 Website

- The official marketing site lives in the `website/` directory and is built with [Astro](https://astro.build/).
- Local development:
  ```bash
  bun run website:dev
  ```
- Production build:
  ```bash
  bun run website:build
  ```

### Release Sync Mechanism

- The website fetches the latest release from:
  `https://api.github.com/repos/codeErrorSleep/dbpaw/releases/latest`
- Version and download links on the website are generated from GitHub release assets.
- If GitHub API is unavailable during build, website generation falls back to `website/src/config/fallback.ts`.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ❤️ Thanks

Thanks for trying DbPaw. If you find it useful, please consider giving this repository a star — it helps a lot!
