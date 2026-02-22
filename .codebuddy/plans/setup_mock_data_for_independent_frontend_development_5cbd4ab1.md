---
name: setup_mock_data_for_independent_frontend_development
overview: 为 Tauri 项目设置零依赖的 Mock 数据系统，支持前端独立开发。通过改造现有的 `api.ts` 中的 `invoke` 函数，在非 Tauri 环境下返回 Mock 数据，实现 `bun dev` 和 `bun tauri dev` 的无缝切换。
todos:
  - id: create-mock-layer
    content: 创建 src/services/mocks.ts，实现所有 API 命令的 Mock 数据生成函数，包括 query、metadata、tableData、connections 四个模块的完整 Mock 实现
    status: completed
  - id: modify-api-invoke
    content: 改造 src/services/api.ts 的 invoke 函数，添加 Mock 模式检测逻辑，非 Tauri 环境下根据 VITE_USE_MOCK 环境变量决定使用 Mock 或报错
    status: completed
    dependencies:
      - create-mock-layer
  - id: create-env-mock
    content: 创建 .env.mock 文件，设置 VITE_USE_MOCK=true，并添加其他开发相关的环境变量配置
    status: completed
  - id: update-package-json
    content: 修改 package.json，添加 dev:mock 脚本用于快速启动前端 Mock 模式开发，保持现有 dev 脚本不变
    status: completed
  - id: test-mock-mode
    content: 测试 Mock 模式的完整功能，验证各个 API 端点的 Mock 数据正确返回，确保组件能正确使用 Mock 数据
    status: completed
    dependencies:
      - modify-api-invoke
      - create-env-mock
      - update-package-json
---

## 产品概述

为 Tauri + React 前端项目实现独立开发模式，支持仅启动前端服务进行调试，无需启动完整的 Tauri 应用。

## 核心功能

1. **零依赖 Mock 方案** - 不引入 MSW 或其他库，通过简单的条件分支实现 Mock/真实 API 切换
2. **类型安全** - Mock 数据严格遵循现有 TypeScript 类型定义，确保编译时类型检查
3. **环境变量控制** - 通过 `VITE_USE_MOCK` 环境变量切换 Mock 模式
4. **完整 Mock 数据** - 为所有核心 API（query、metadata、tableData、connections）提供可用的 Mock 数据
5. **快速切换** - 支持快速在 Mock 模式和真实模式之间切换，无需修改代码
6. **开发体验** - 提供 `bun dev:mock` 脚本快速启动前端 Mock 模式

## 应用场景

- 前端独立开发调试，无需等待后端启动
- UI 测试没有后端数据支持的元素显示
- 快速原型验证前端交互逻辑
- 支持离线开发

## 技术栈选择

- **开发服务器**: Vite（已有，保持不变）
- **运行时检测**: 使用 `__TAURI_INTERNALS__` 和环境变量判断运行环境
- **类型系统**: TypeScript 类型复用（零额外依赖）
- **环境管理**: Vite 原生 `import.meta.env` 系统

## 实现方案

### 核心思路

采用**环境变量驱动的条件式 Mock**方案：

- 在非 Tauri 环境下，根据 `VITE_USE_MOCK` 环境变量决定是否使用 Mock 数据
- Mock 数据直接返回 TypeScript 类型定义的数据结构
- 无需修改现有组件代码，所有适配在 `api.ts` 和 `mocks.ts` 中进行
- 保持现有 Tauri 调用逻辑不变

### 架构设计

```mermaid
graph LR
    A[React 组件] -->|调用| B[api 对象]
    B -->|isTauri() && !useMock| C[Tauri invoke<br/>真实后端]
    B -->|!isTauri() && useMock| D[Mock 数据层<br/>src/services/mocks.ts]
    B -->|!isTauri() && !useMock| E[错误提示]
    C -->|返回 QueryResult| F[类型安全]
    D -->|返回 QueryResult| F
```

### 关键决策

1. **Mock 层隔离** - 所有 Mock 逻辑独立在 `mocks.ts`，便于维护和扩展
2. **条件判断顺序** - 先检查 Tauri，再检查 Mock 模式，优先级清晰
3. **环境变量方案** - 使用 `VITE_USE_MOCK` 和 `.env.mock` 文件，符合 Vite 约定
4. **脚本简化** - 添加 `dev:mock` 脚本，一键启动 Mock 模式

## 实现细节

### 修改 src/services/api.ts

- 在 `invoke` 函数中添加 Mock 模式检测
- 非 Tauri 环境且 `VITE_USE_MOCK` 为 true 时，调用 Mock 数据层
- 保持现有 Tauri 调用逻辑不变

### 创建 src/services/mocks.ts

- 定义完整的 Mock 数据生成函数，覆盖所有 API 命令
- 每个 Mock 函数返回符合真实类型定义的数据
- 可模拟不同场景：成功、失败、不同数据量等
- 数据结构示例：
- `QueryResult`: 包含查询结果、列信息、执行时间
- `TableMetadata`: 包含列、索引、外键信息
- `SchemaOverview`: 包含表结构概览
- 连接列表和连接信息

### 创建 .env.mock 文件

- 设置 `VITE_USE_MOCK=true`

### 修改 package.json

- 添加 `dev:mock` 脚本：`dotenv -e .env.mock -- vite` 或 `VITE_USE_MOCK=true vite`
- 保持现有 `dev` 脚本不变

## 性能考虑

- Mock 数据直接返回，无网络延迟，调试快速
- 可选添加模拟延迟（`setTimeout`）以测试加载状态
- 内存占用极小，仅在非 Tauri 环境下加载 Mock 数据

## 类型安全保证

- 所有 Mock 函数返回值严格匹配 `api.ts` 中定义的类型
- TypeScript 编译时检查确保类型安全
- Mock 和真实数据可无缝切换