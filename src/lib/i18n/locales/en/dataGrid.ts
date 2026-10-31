export const datagrid = {
  viewer: {
    copied: "Copied",
    copyJson: "Copy JSON",
    typeArray: "array",
    typeObject: "object",
  },
} as const;

export const tableView = {
  columnSearch: {
    title: "Find field",
    placeholder: "Field name or comment...",
    hint: "Find fields by name or comment. Enter: next; Shift+Enter: previous.",
    count: "Field {{current}} of {{total}}",
    noMatches: "No matching fields",
    previous: "Previous matching field",
    next: "Next matching field",
    clear: "Clear field search",
  },
  toolbar: {
    totalOn: "Exact total is on",
    totalOff: "Exact total is off",
  },
  header: {
    actionHint: "Click to copy column name, double-click to sort {{column}}",
  },
  toast: {
    columnNameCopied: "Copied column name: {{column}}",
    copyFailed: "Failed to copy",
    cellCopied: "Cell copied",
    cellsCopied: "Copied {{rows}}×{{columns}} cells",
    cellsPasted: "Pasted {{count}} cell(s)",
  },
  deleteRows: {
    title: "Delete selected rows?",
    description:
      "This action will permanently delete {{count}} row(s) from the table.",
    deleting: "Deleting...",
  },
  contextMenu: {
    copySelection: "Copy Selection",
    copyCell: "Copy Cell",
    copySelectionAs: "Copy Selection as",
    selectionCopiedAsCsv: "Selection copied as CSV",
    selectionCopiedAsInsertSql: "Selection copied as Insert SQL",
    selectionCopiedAsUpdateSql: "Selection copied as Update SQL",
    copiedRows: "Copied {{count}} row(s)",
    rowCopied: "Row copied",
    copySelectedRows: "Copy Selected Rows",
    copyRow: "Copy Row",
    undoThisCell: "Undo This Cell",
    copyAs: "Copy as",
    copiedAsCsv: "Copied as CSV",
    rowCopiedAsCsv: "Row copied as CSV",
    copySelectedAsCsv: "Copy Selected as CSV",
    copyAsCsv: "Copy as CSV",
    copiedAsInsertSql: "Copied as Insert SQL",
    rowCopiedAsInsertSql: "Row copied as Insert SQL",
    copySelectedAsInsertSql: "Copy Selected as Insert SQL",
    copyAsInsertSql: "Copy as Insert SQL",
    copiedAsUpdateSql: "Copied as Update SQL",
    rowCopiedAsUpdateSql: "Row copied as Update SQL",
    copySelectedAsUpdateSql: "Copy Selected as Update SQL",
    copyAsUpdateSql: "Copy as Update SQL",
  },
} as const;

export const tableSelector = {
  emptyLabel: "Select table schema (no data)",
  selectedLabel: "Schema: {{count}} selected",
  triggerAria: "Only schema, no data",
  clearSelection: "Clear table selection",
  searchTables: "Find tables...",
  noTablesFound: "No tables found",
  tablesHeading: "Tables",
} as const;
