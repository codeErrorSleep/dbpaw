# 開発ガイド

## 前提条件

- Rust（最新の stable）
- Bun（推奨）または Node.js（v18+）
- Tauri に必要なプラットフォームツールチェーン：https://tauri.app/start/prerequisites/

## セットアップ

```bash
git clone https://github.com/codeErrorSleep/dbpaw.git
cd dbpaw
bun install
```

## 起動

フロントエンドのみ（Mock モード）— UI 開発に推奨：

```bash
bun dev:mock
```

フルアプリ（Tauri + Rust）— E2E の動作確認向け：

```bash
bun tauri dev
```

## ビルド

```bash
bun tauri build
```

## テスト

すべて実行：

```bash
bun run test:all
```

個別に実行：

```bash
bun run test:unit
bun run test:service
bun run test:rust:unit
bun run test:integration
```

## フォーマット

```bash
bun run format
```

## 🌐 Web サイト

- 公式マーケティングサイトは `website/` ディレクトリにあり、[Astro](https://astro.build/) で構築されています。
- ローカル開発：
  ```bash
  bun run website:dev
  ```
- プロダクションビルド：
  ```bash
  bun run website:build
  ```

### リリース同期メカニズム

- Web サイトは以下から最新リリースを取得します：
  `https://api.github.com/repos/codeErrorSleep/dbpaw/releases/latest`
- Web サイト上のバージョンとダウンロードリンクは GitHub リリースのアセットから自動生成されます。
- ビルド時に GitHub API が利用できない場合、Web サイトは `website/src/config/fallback.ts` にフォールバックします。
