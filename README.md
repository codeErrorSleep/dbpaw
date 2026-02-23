# DbPaw

![DbPaw Logo](public/product-icon.png)

> **A modern, AI-powered database client for the new era.**

[![Release](https://img.shields.io/github/v/release/username/DbPaw?style=flat-square)](https://github.com/username/DbPaw/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg?style=flat-square)](https://tauri.app)

**DbPaw** is a lightweight, cross-platform database management tool built with [Tauri](https://tauri.app) and [React](https://react.dev). It combines the performance of a native Rust backend with a beautiful, modern web-based UI to provide a seamless SQL editing and data exploration experience.

## ✨ Features

- **🔌 Multi-Database Support**: Connect to **PostgreSQL** and **MySQL** databases with ease.
- **🤖 AI-Powered Assistance**: Integrated AI sidebar to help you write complex SQL queries, explain execution plans, and optimize performance.
- **🔒 Secure Connectivity**: Support for **SSH Tunneling** to securely access remote databases.
- **📝 Advanced SQL Editor**: 
  - Syntax highlighting and auto-completion.
  - **Saved Queries** library to organize your frequently used scripts.
  - Format SQL with a single click.
- **📊 Interactive Data Grid**: 
  - View, filter, and sort table data efficiently.
  - Visualize data relationships.
- **🎨 Modern UI**: 
  - Beautifully designed with **TailwindCSS** and **Shadcn/UI**.
  - Built-in **Dark Mode** support for comfortable coding at night.
- **🚀 High Performance**: Built on Rust, ensuring low memory usage and blazing fast startup times.

## 📥 Installation

Go to the [Releases](https://github.com/username/DbPaw/releases) page to download the latest version for your operating system.

### macOS Users
If you encounter a "DbPaw is damaged" or "Unidentified Developer" warning upon opening the app:

1. Move `DbPaw.app` to your `/Applications` folder.
2. Open **Terminal** and run the following command:
   ```bash
   sudo xattr -d com.apple.quarantine /Applications/DbPaw.app
   ```
3. You can now open the app normally.

*Note: This is required because the app is not yet notarized by Apple.*

## 🛠️ Development

If you want to contribute or build from source, follow these steps:

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Bun](https://bun.sh/) or Node.js (v18+)

### Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/username/DbPaw.git
   cd DbPaw
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

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
