export const datagrid = {
  viewer: {
    copied: "已复制",
    copyJson: "复制 JSON",
    typeArray: "数组",
    typeObject: "对象",
  },
} as const;

export const tableView = {
  columnSearch: {
    title: "查找字段",
    placeholder: "字段名或注释...",
    hint: "按字段名或注释查找。Enter：下一个；Shift+Enter：上一个。",
    count: "第 {{current}} / {{total}} 个字段",
    noMatches: "未找到匹配字段",
    previous: "上一个匹配字段",
    next: "下一个匹配字段",
    clear: "清空字段搜索",
  },
  toolbar: {
    totalOn: "已开启精确总量",
    totalOff: "已关闭精确总量",
  },
  header: {
    actionHint: "单击复制字段名，双击按 {{column}} 排序",
  },
  toast: {
    columnNameCopied: "已复制字段名：{{column}}",
    copyFailed: "复制失败",
    cellCopied: "单元格已复制",
    cellsCopied: "已复制 {{rows}}×{{columns}} 个单元格",
    cellsPasted: "已粘贴 {{count}} 个单元格",
  },
  deleteRows: {
    title: "删除选中的行？",
    description: "此操作将从表中永久删除 {{count}} 行。",
    deleting: "删除中...",
  },
  contextMenu: {
    copySelection: "复制选区",
    copyCell: "复制单元格",
    copySelectionAs: "复制选区为",
    selectionCopiedAsCsv: "选区已复制为 CSV",
    selectionCopiedAsInsertSql: "选区已复制为 Insert SQL",
    selectionCopiedAsUpdateSql: "选区已复制为 Update SQL",
    copiedRows: "已复制 {{count}} 行",
    rowCopied: "行已复制",
    copySelectedRows: "复制选中行",
    copyRow: "复制行",
    undoThisCell: "撤销此单元格",
    copyAs: "复制为",
    copiedAsCsv: "已复制为 CSV",
    rowCopiedAsCsv: "行已复制为 CSV",
    copySelectedAsCsv: "复制选中行为 CSV",
    copyAsCsv: "复制为 CSV",
    copiedAsInsertSql: "已复制为 Insert SQL",
    rowCopiedAsInsertSql: "行已复制为 Insert SQL",
    copySelectedAsInsertSql: "复制选中行为 Insert SQL",
    copyAsInsertSql: "复制为 Insert SQL",
    copiedAsUpdateSql: "已复制为 Update SQL",
    rowCopiedAsUpdateSql: "行已复制为 Update SQL",
    copySelectedAsUpdateSql: "复制选中行为 Update SQL",
    copyAsUpdateSql: "复制为 Update SQL",
  },
} as const;

export const tableSelector = {
  emptyLabel: "选择表结构（不含数据）",
  selectedLabel: "已选结构：{{count}} 项",
  triggerAria: "仅包含结构，不包含数据",
  clearSelection: "清空表选择",
  searchTables: "搜索数据表...",
  noTablesFound: "未找到数据表",
  tablesHeading: "数据表",
} as const;
