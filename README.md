# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## 🚀 Getting Started

### Development Modes

This project supports two development modes for optimal workflow:

#### 1. Frontend-Only Development (Mock Mode) ⭐ Recommended for UI Development
```bash
bun dev:mock
```
- **Fast startup** (2-3 seconds)
- **Hot reload** for immediate frontend changes
- **Mock data** provides complete UI experience without backend
- **Perfect for**: UI components, styling, layout work

#### 2. Full Application Development
```bash
bun tauri dev
```
- **Complete application** with Rust backend
- **Real data** from database connections
- **Full integration testing**
- **Perfect for**: API testing, database operations, final testing

### Environment Variables

The project uses environment variables to control development behavior:

- `VITE_USE_MOCK=true` - Enables mock mode for frontend-only development
- `.env.mock` - Pre-configured environment file for mock mode

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
