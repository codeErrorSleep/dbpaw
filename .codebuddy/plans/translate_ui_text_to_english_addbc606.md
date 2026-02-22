---
name: translate_ui_text_to_english
overview: 将应用中的所有中文提示词翻译成英文，包括设置对话框和主应用中的相关文本。
todos:
  - id: translate-app-tsx
    content: 翻译 App.tsx：用户菜单、AI面板、标签页操作菜单的中文文本为英文
    status: completed
  - id: translate-settings-dialog
    content: 翻译 SettingsDialog.tsx：设置对话框标题、描述、主题选项、关于部分的中文文本为英文
    status: completed
  - id: translate-database-sidebar
    content: 翻译 DatabaseSidebar.tsx：数据库连接对话框的验证消息、测试提示、右键菜单操作的中文文本为英文
    status: completed
  - id: translate-table-view
    content: 翻译 TableView.tsx：保存、撤销、只读提示等表格操作的中文文本为英文
    status: completed
  - id: verify-translation
    content: 验证所有翻译是否正确，检查是否有遗漏的中文文本
    status: completed
    dependencies:
      - translate-app-tsx
      - translate-settings-dialog
      - translate-database-sidebar
      - translate-table-view
---

## 用户需求

将前端应用界面中的所有中文提示词、标签和菜单文本转换为英文。目标是支持国际化，使应用更适合全球用户。

## 核心功能

- **用户菜单翻译**：将顶部用户菜单项翻译为英文（"我的账号" → "My Account", "个人资料" → "Profile", "设置" → "Settings", "退出登录" → "Logout"）
- **标签页操作翻译**：将右键菜单操作翻译为英文（"关闭当前标签" → "Close Tab", "关闭其他标签" → "Close Other Tabs"）
- **设置对话框翻译**：翻译设置界面的所有文本（标题、描述、主题模式、强调色等）
- **数据表操作翻译**：翻译表格操作按钮和提示文本（保存、撤销、只读等）
- **数据库操作翻译**：翻译数据库连接界面的菜单项和验证提示
- **AI面板翻译**：翻译AI面板的显示/隐藏提示文本

## 技术栈选择

当前项目使用React + TypeScript + Tailwind CSS + shadcn/ui组件库。无需添加国际化库，直接替换硬编码的中文字符串为英文。

## 实现方案

采用直接替换策略，逐个文件地将中文字符串替换为对应的英文表达。由于应用规模不大，中文字符串有限且分散，直接替换是最高效的解决方案，无需引入i18n库。

### 技术决策

1. **直接字符串替换**：在各个组件中直接修改中文字符串为英文，无需引入额外库
2. **保留现有架构**：不改动组件结构、样式或逻辑，只修改UI文本
3. **分文件处理**：按照文件修改，确保每次改动都是独立、可追踪的

### 性能考虑

- 无性能影响，仅为文本替换操作
- 编译过程无变化

### 执行细节

- **App.tsx**：修改用户菜单标签、AI面板标题、标签页右键菜单
- **SettingsDialog.tsx**：修改对话框标题、描述、所有主题和外观相关的文本
- **DatabaseSidebar.tsx**：修改数据库连接对话框的验证消息、测试提示、右键菜单操作文本
- **TableView.tsx**：修改表格操作按钮标签、错误提示、保存/撤销按钮文本、表格状态提示