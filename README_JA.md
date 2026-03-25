<div align="center">
  <img align="center" src="./public/product-icon.png" width="120" height="120" />
</div>

<h2 align="center">DbPaw</h2>

<div align="center">
<br>
<em>より速い SQL 編集とデータ探索体験 ― クロスプラットフォーム、超軽量、AI アシスタントはオプション。</em>
<br><br>

[English](README.md) | [简体中文](README_CN.md) | 日本語

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

**DbPaw** は PostgreSQL / MySQL / MariaDB（MySQL 互換）/ TiDB（MySQL 互換）/ SQLite / SQL Server / ClickHouse（プレビュー）に接続し、SQL の作成・実行とデータ確認を、クリーンなデスクトップ UI で快適に行えます。

## ✅ できること

- PostgreSQL、MySQL、MariaDB（MySQL 互換）、TiDB（MySQL 互換）、SQLite、SQL Server、ClickHouse（プレビュー、現状は読み取り専用）に接続
- SQL の作成・実行（シンタックスハイライト、補完、ワンクリック整形）
- データグリッドで結果を閲覧（フィルタ、ソート、ページネーション、エクスポート）
- `Saved Queries` でよく使う SQL を保存・再利用
- AI サイドバーで SQL 作成補助やクエリ説明（任意）
- SSH トンネル経由でリモート DB に安全に接続

## 🖼️ スクリーンショット

![DbPaw Main Workspace](docs/screenshots/01-overview.png.png)

![DbPaw Main Workspace (Dark)](docs/screenshots/01-overview-black.png)

| 接続管理                                       | SQL エディタ                              |
| ---------------------------------------------- | ----------------------------------------- |
| ![Connection](docs/screenshots/02-connect.png) | ![Editor](docs/screenshots/03-editor.png) |

| データグリッド                            | AI アシスタント                   |
| ----------------------------------------- | --------------------------------- |
| ![Grid](docs/screenshots/04-ddl-grid.png) | ![AI](docs/screenshots/05-ai.png) |

## ✨ 主な機能

- **超軽量**: インストーラは約 10MB、インストール後は約 80MB。常駐メモリも極小（Electron 系ツールを大きく下回ります）。
- **真にモダン**: DBeaver の「コックピット」的な複雑 UI から卒業。開発者が一生使わない 99% をそぎ落とし、よく使うシナリオに集中。より直感的でスムーズな操作感。
- **クロスプラットフォーム**: macOS / Windows / Linux をサポート（職場と自宅で別アプリを使い分ける必要なし）。
- **DB 互換**: 現在は MySQL、MariaDB（MySQL 互換）、PostgreSQL、ClickHouse、TiDB、SQL Server、SQLite に対応（対応拡大中）。
- **ルックス良し**: 多数のテーマ（ダーク/ライト、高彩度/低彩度など）を同梱。
- **組み込み AI 支援（実験的）**: SQL 要約、スキーマ説明、遅いクエリ分析などに対応（セキュリティは継続改善中。今後はローカル/選択式クラウドモードを追加予定）。
- **完全無料**: ログイン不要、課金なし、会員機能なし、広告なし。

## 📥 インストール

[Releases](https://github.com/codeErrorSleep/dbpaw/releases) から、お使いの OS 向け最新バージョンをダウンロードしてください。

### macOS

1. [Releases](https://github.com/codeErrorSleep/dbpaw/releases) から macOS 版 `DbPaw` をダウンロード
2. `DbPaw.app` を `/Applications` フォルダへ移動
3. アプリを起動

macOS で「未確認の開発元」と表示されて起動をブロックされた場合:

1. **システム設定** → **プライバシーとセキュリティ** を開く
2. **セキュリティ** セクションで `DbPaw` がブロックされた旨の表示を探す
3. **このまま開く** をクリックし、確認ダイアログで **開く** を選択

「DbPaw は破損しているため開けません」（Gatekeeper の隔離属性）と表示される場合:

1. `DbPaw.app` を `/Applications` フォルダへ移動
2. **ターミナル**を開き、次を実行
   ```bash
   sudo xattr -d com.apple.quarantine /Applications/DbPaw.app
   ```
3. 通常どおり起動可能になります

_注: 現時点では Apple の notarization（公証）が未完了のため、この対応が必要になる場合があります。_

### Windows

1. [Releases](https://github.com/codeErrorSleep/dbpaw/releases) からインストーラまたはポータブル版をダウンロード
2. インストーラ / 実行ファイルを起動

「Windows によって PC が保護されました」（SmartScreen）などの警告が表示される場合:

1. **詳細情報** をクリック
2. **実行** をクリック

組織管理端末の場合、IT 管理者による許可が必要なことがあります。

## 🔐 セキュリティとプライバシー

- DbPaw はローカルデスクトップアプリです。DB 接続はあなたの端末から直接データベースへ行われます。
- AI 機能は任意です。有効化すると、プロンプト、直近の会話コンテキスト、必要に応じてスキーマ概要（テーブル/カラム/型）を、設定した AI プロバイダーへ送信します。
- AI 会話はローカルに保存され、AI プロバイダーの API キーはローカルディスク上で暗号化保存されます。
- デスクトップアプリには、標準でテレメトリ / 分析 SDK は組み込まれていません。

## 🛠️ 開発

- 開発ガイド：[docs/ja/Development/DEVELOPMENT.md](docs/ja/Development/DEVELOPMENT.md)
- コントリビューション：[docs/ja/Community/CONTRIBUTING.md](docs/ja/Community/CONTRIBUTING.md)

## 🏗️ 技術スタック

- **Core**: [Tauri v2](https://v2.tauri.app/)（Rust）
- **Frontend**: [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/)
- **Styling**: [TailwindCSS v4](https://tailwindcss.com/), [Shadcn/UI](https://ui.shadcn.com/)
- **State Management**: React Hooks & Context
- **Editor**: [Monaco Editor](https://microsoft.github.io/monaco-editor/) / CodeMirror

## 📄 ライセンス

このプロジェクトは Apache License 2.0 のもとで提供されています。詳細は [LICENSE](LICENSE) を参照してください。

## ❤️ Thanks

DbPaw を試していただきありがとうございます。役に立った場合は、ぜひリポジトリに Star をお願いします。
