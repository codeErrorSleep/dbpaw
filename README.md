<div align="center">
  <img align="center" src="./public/product-icon.png" width="120" height="120" />
</div>

<h2 align="center">DbPaw</h2>

<div align="center">
<br>
<em>Faster SQL editing and data exploration — cross-platform, ultra-lightweight, with optional AI assistance.</em>
<br><br>

English | [简体中文](README_CN.md) | [日本語](README_JA.md)

</div>

<div align="center">

[![GitHub Repo stars](https://img.shields.io/github/stars/codeErrorSleep/dbpaw?style=flat-square)](https://github.com/codeErrorSleep/dbpaw)
[![G-Star](https://atomgit.com/codeErrorSleep/dbpaw/star/badge.svg)](https://atomgit.com/codeErrorSleep/dbpaw)
[![Release](https://img.shields.io/github/v/release/codeErrorSleep/dbpaw?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/releases)
[![Downloads](https://img.shields.io/github/downloads/codeErrorSleep/dbpaw/total?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg?style=flat-square)](https://tauri.app)
<br/>
[![TypeScript](https://img.shields.io/badge/TypeScript-5.8-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue?style=flat-square&logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/pulls)

</div>

**DbPaw** helps you connect to PostgreSQL, MySQL, MariaDB (MySQL-compatible), TiDB (MySQL-compatible), SQLite, SQL Server, and ClickHouse (preview), write and run SQL efficiently, and inspect data in a clean desktop UI.

## ✅ What You Can Do

- Connect to PostgreSQL, MySQL, MariaDB (MySQL-compatible), TiDB (MySQL-compatible), SQLite, SQL Server, and ClickHouse (preview, currently read-only)
- Write and run SQL with syntax highlighting, auto-completion, and one-click formatting
- Browse query results in a data grid with filtering, sorting, pagination, and export
- Save and reuse frequently used SQL scripts with Saved Queries
- Use the AI sidebar to draft SQL and explain queries (optional)
- Access remote databases through SSH tunneling

## 🖼️ Screenshots

![DbPaw Main Workspace](docs/screenshots/01-overview.png.png)

![DbPaw Main Workspace (Dark)](docs/screenshots/01-overview-black.png)

| Connection                                     | SQL Editor                                |
| ---------------------------------------------- | ----------------------------------------- |
| ![Connection](docs/screenshots/02-connect.png) | ![Editor](docs/screenshots/03-editor.png) |

| Data Grid                                 | AI Assistant                      |
| ----------------------------------------- | --------------------------------- |
| ![Grid](docs/screenshots/04-ddl-grid.png) | ![AI](docs/screenshots/05-ai.png) |

## ✨ Features

- **Tiny footprint**: installer ≈10 MB, on-disk ≈80 MB, and very low idle memory (much lighter than Electron-based tools).
- **Truly modern**: goodbye to DBeaver-style “cockpit” UIs—we cut the 99% you’ll never use, focus on common workflows, and make every action smoother and more intuitive.
- **Cross-platform**: runs on macOS, Windows, and Linux (no more one app at work and another at home).
- **Database compatibility**: currently supports MySQL, MariaDB (MySQL-compatible), PostgreSQL, ClickHouse, TiDB, SQL Server, and SQLite (actively expanding).
- **Looks great**: lots of themes (dark/light and a range of high/low saturation styles).
- **Built-in AI assistance (experimental)**: summarize SQL, explain schemas, and analyze slow queries with AI (security under active refinement; local/optional cloud modes planned).
- **Completely free**: no login, no payments, no memberships, no ads.

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

- Development guide: [docs/en/Development/DEVELOPMENT.md](docs/en/Development/DEVELOPMENT.md)
- Contributing guide: [docs/en/Community/CONTRIBUTING.md](docs/en/Community/CONTRIBUTING.md)

## 🏗️ Tech Stack

- **Core**: [Tauri v2](https://v2.tauri.app/) (Rust)
- **Frontend**: [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/)
- **Styling**: [TailwindCSS v4](https://tailwindcss.com/), [Shadcn/UI](https://ui.shadcn.com/)
- **State Management**: React Hooks & Context
- **Editor**: [Monaco Editor](https://microsoft.github.io/monaco-editor/) / CodeMirror

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## ❤️ Thanks

Thanks for trying DbPaw. If you find it useful, please consider giving this repository a star — it helps a lot!
