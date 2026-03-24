# Development

## Prerequisites

- Rust (latest stable)
- Bun (recommended) or Node.js (v18+)
- Platform toolchain required by Tauri: https://tauri.app/start/prerequisites/

## Setup

```bash
git clone https://github.com/codeErrorSleep/dbpaw.git
cd dbpaw
bun install
```

## Run

Frontend-only (Mock Mode) — recommended for UI work:

```bash
bun dev:mock
```

Full app (Tauri + Rust) — for end-to-end testing:

```bash
bun tauri dev
```

## Build

```bash
bun tauri build
```

## Tests

Run everything:

```bash
bun run test:all
```

Or run a subset:

```bash
bun run test:unit
bun run test:service
bun run test:rust:unit
bun run test:integration
```

## Formatting

```bash
bun run format
```

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
