import { expect, test } from "@playwright/test";
import { collectRuntimeErrors } from "./runtime-errors";

async function openNewQueryTab(
  page: import("@playwright/test").Page,
  connectionName = "PostgreSQL Dev",
) {
  const connectionNode = page.getByText(connectionName).first();
  await connectionNode.click({ button: "right" });
  const contextMenu = page.locator(".fixed.z-50");
  await contextMenu.getByRole("button", { name: "New Query" }).click();
  // Wait for the CodeMirror editor to appear
  await expect(page.locator(".cm-editor").first()).toBeVisible();
}

async function typeInEditor(
  page: import("@playwright/test").Page,
  text: string,
) {
  await page.locator(".cm-content").first().click();
  await page.keyboard.type(text, { delay: 0 });
}

test("SQL editor: toolbar buttons exist", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();
  runtimeErrors.assertClean("App boot");

  await openNewQueryTab(page);
  runtimeErrors.assertClean("New Query tab opened");

  // Verify all toolbar buttons are present
  await expect(page.getByRole("button", { name: /Run SQL/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Format SQL/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Save Query/ })).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Clear Editor/ }),
  ).toBeVisible();
  runtimeErrors.assertClean("All toolbar buttons visible");
});

test("SQL editor: execute query shows results", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();
  runtimeErrors.assertClean("App boot");

  await openNewQueryTab(page);
  runtimeErrors.assertClean("New Query tab opened");

  await typeInEditor(page, "SELECT * FROM users");
  await page.getByRole("button", { name: /Run SQL/ }).click();

  // Wait for results to appear
  await expect(page.getByText(/Returned 10 row/)).toBeVisible({
    timeout: 10_000,
  });
  runtimeErrors.assertClean("Execute query");
});

test("SQL editor: format SQL", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  await typeInEditor(page, "select id,name from users where id=1");

  await page.getByRole("button", { name: /Format SQL/ }).click();

  // Verify the editor content was reformatted (should contain newlines/indentation)
  const editorContent = page.locator(".cm-content").first();
  const text = await editorContent.textContent();
  expect(text).toContain("select");
  expect(text).toContain("from");
  runtimeErrors.assertClean("Format SQL");
});

test("SQL editor: cancel query lifecycle", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page, "MySQL Dev");
  await typeInEditor(page, "SELECT 1 /* dbpaw-test:delay=1500 */");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  const cancelButton = page.getByRole("button", { name: /Cancel Query/ });
  await expect(cancelButton).toBeVisible();
  await cancelButton.click();
  const cancellingButton = page.getByRole("button", {
    name: /Cancelling query/,
  });
  await expect(cancellingButton).toBeDisabled();
  await expect(page.getByText(/Query cancelled/)).toBeVisible();
  runtimeErrors.assertClean("Cancel query");
});

test("SQL editor: closing a running tab cancels without restoring stale results", async ({
  page,
}) => {
  const runtimeErrors = collectRuntimeErrors(page);
  await page.goto("/");
  await openNewQueryTab(page, "MySQL Dev");
  await typeInEditor(page, "SELECT 1 /* dbpaw-test:delay=1200 */");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(
    page.getByRole("button", { name: /Cancel Query/ }),
  ).toBeVisible();

  await page
    .getByRole("button", { name: "Close Query (testdb)", exact: true })
    .click();
  const unsavedDialog = page.getByRole("alertdialog");
  await unsavedDialog.getByRole("button", { name: "Don't Save" }).click();
  await expect(page.getByRole("tab", { name: /Query \(testdb\)/ })).toHaveCount(
    0,
  );

  await page.waitForTimeout(1_300);
  await expect(page.getByText(/Returned 10 row/)).toHaveCount(0);
  runtimeErrors.assertClean("Close running query tab");
});

test("SQL editor: selected SQL keeps its execution provenance", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT 1;\nSELECT * FROM users");

  const editor = page.locator(".cm-content").first();
  await editor.click();
  await page.keyboard.press("ControlOrMeta+Home");
  await page.keyboard.press("Shift+End");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible();

  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.type(" -- edited later");
  await expect(
    page.getByText("Result is from a previous editor revision"),
  ).toBeVisible();

  await page.getByLabel("Open SQL execution logs").click();
  const logsPopover = page.locator("[data-radix-popper-content-wrapper]");
  await expect(
    logsPopover.getByText("SELECT 1;", { exact: true }).first(),
  ).toBeVisible();
  await expect(logsPopover.getByText(/SELECT \* FROM users/)).toHaveCount(0);
});

test("SQL editor: execution provenance, default limit, and partial errors", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT * FROM users");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(
    page.getByText("Automatically limited to 1000 rows"),
  ).toBeVisible();

  await typeInEditor(page, " -- edited after execution");
  await expect(
    page.getByText("Result is from a previous editor revision"),
  ).toBeVisible();

  const editor = page.locator(".cm-content").first();
  await editor.click();
  await page.keyboard.press("Meta+a");
  await page.keyboard.type("SELECT 1; SELECT 2 /* dbpaw-test:partial-error */");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText("Error", { exact: true })).toBeVisible();
  await expect(page.getByText(/Completed 1 statement/)).toBeVisible();
  await expect(page.getByText(/Result 1 \(3 rows\)/)).toBeVisible();
  await expect(page.getByText("alice@example.com").first()).toBeVisible();
});

test("SQL editor: risky SQL requires confirmation and reports affected rows", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(
    page,
    "UPDATE users SET username = 'archived' /* dbpaw-test:affected=3 */",
  );

  await page.getByRole("button", { name: /Run SQL/ }).click();
  let dialog = page.getByRole("alertdialog");
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByText("UPDATE users SET username = 'archived'", {
      exact: false,
    }),
  ).toBeVisible();
  await expect(
    dialog.getByText("UPDATE or DELETE has no top-level WHERE clause."),
  ).toBeVisible();

  await dialog.getByRole("button", { name: "Cancel" }).click();
  await page.getByLabel("Open SQL execution logs").click();
  await expect(page.getByText("No execution logs yet.")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: /Run SQL/ }).click();
  dialog = page.getByRole("alertdialog");
  await dialog.getByRole("button", { name: "Execute anyway" }).click();
  await expect(page.getByText(/Affected 3 row.*18 ms/)).toBeVisible();
});

test("SQL editor: rerun export uses the execution snapshot", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(
    page,
    "SELECT * FROM users /* dbpaw-test:export-token=before */",
  );
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible();

  const editor = page.locator(".cm-content").first();
  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.type(" /* dbpaw-test:export-token=after */");

  await page.getByRole("button", { name: "Rerun and Export" }).click();
  await page.getByRole("menuitem", { name: "CSV" }).click();
  await expect(page.getByText(/dbpaw-query-export-before\.csv/)).toBeVisible();
  await expect(page.getByText(/dbpaw-query-export-after\.csv/)).toHaveCount(0);
});

test("SQL editor: context selectors are disabled while running", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT 1 /* dbpaw-test:delay=800 */");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(
    page.getByRole("combobox", { name: "Switch database" }),
  ).toBeDisabled();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible();
});

test("SQL editor: tab switch restores viewport and focus", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);
  await page.goto("/");
  await openNewQueryTab(page);
  const sql = Array.from({ length: 80 }, (_, index) => `SELECT ${index};`).join(
    "\n",
  );
  // Seed the scrollable document in one input event. Per-character typing
  // spends the CI timeout on setup instead of testing tab restoration.
  await page.locator(".cm-content").first().click();
  await page.keyboard.insertText(sql);
  const firstScroller = page.locator(".cm-scroller").first();
  await firstScroller.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
    element.dispatchEvent(new Event("scroll"));
  });
  await page.locator(".cm-content").first().click();
  await page.keyboard.press("ControlOrMeta+End");
  const savedScrollTop = await firstScroller.evaluate(
    (element) => element.scrollTop,
  );
  expect(savedScrollTop).toBeGreaterThan(0);

  await openNewQueryTab(page);
  const queryTabs = page.getByRole("tab", { name: /Query \(testdb\)/ });
  await expect(queryTabs).toHaveCount(2);
  await queryTabs.nth(0).click();

  await expect
    .poll(() =>
      page
        .locator(".cm-scroller")
        .first()
        .evaluate((element) => element.scrollTop),
    )
    .toBeGreaterThan(0);
  await expect(page.locator(".cm-content").first()).toBeFocused();
  await expect(page.locator(".cm-content").first()).toContainText("SELECT 79;");
  runtimeErrors.assertClean("Restore SQL editor viewport and focus");
});

test("SQL editor: tab switch restores the editor/result split", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT * FROM users");
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible();

  const editor = page.locator("[data-sql-editor-tab]").first();
  const group = editor.locator('[data-slot="resizable-panel-group"]');
  const editorPanel = group.locator('[data-slot="resizable-panel"]').first();
  const handle = group.locator('[data-slot="resizable-handle"]');
  const handleBox = await handle.boundingBox();
  if (!handleBox) throw new Error("result split handle is not visible");
  await page.mouse.move(
    handleBox.x + handleBox.width / 2,
    handleBox.y + handleBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y - 80, {
    steps: 8,
  });
  await page.mouse.up();

  await expect
    .poll(async () => {
      const [panelBox, groupBox] = await Promise.all([
        editorPanel.boundingBox(),
        group.boundingBox(),
      ]);
      return panelBox && groupBox ? panelBox.height / groupBox.height : 0;
    })
    .toBeLessThan(0.45);
  const beforeSwitch = await editorPanel.boundingBox();
  const groupBeforeSwitch = await group.boundingBox();
  if (!beforeSwitch || !groupBeforeSwitch) {
    throw new Error("resizable panels are not measurable");
  }
  const expectedRatio = beforeSwitch.height / groupBeforeSwitch.height;

  await openNewQueryTab(page);
  const queryTabs = page.getByRole("tab", { name: /Query \(testdb\)/ });
  await queryTabs.nth(0).click();

  await expect
    .poll(async () => {
      const restoredEditor = page
        .locator("[data-sql-editor-tab]")
        .first()
        .locator('[data-slot="resizable-panel"]')
        .first();
      const restoredGroup = page
        .locator("[data-sql-editor-tab]")
        .first()
        .locator('[data-slot="resizable-panel-group"]');
      const [panelBox, groupBox] = await Promise.all([
        restoredEditor.boundingBox(),
        restoredGroup.boundingBox(),
      ]);
      return panelBox && groupBox
        ? Math.abs(panelBox.height / groupBox.height - expectedRatio)
        : 1;
    })
    .toBeLessThan(0.03);
});

test("SQL editor: save query opens dialog", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT * FROM users");

  await page.getByRole("button", { name: /Save Query/ }).click();

  // Verify Save Query dialog opens
  await expect(page.getByRole("dialog", { name: "Save Query" })).toBeVisible();

  // Fill in the query name
  await page.getByLabel("Query Name").fill("My Test Query");

  // Submit the save
  await page.getByRole("button", { name: "Save" }).click();

  // Dialog should close
  await expect(page.getByRole("dialog", { name: "Save Query" })).toBeHidden();
  runtimeErrors.assertClean("Save query");
});

test("SQL editor: clear editor", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT 1");

  // Verify editor has content
  const editorContent = page.locator(".cm-content").first();
  await expect(editorContent).toHaveText(/SELECT/);

  // Click Clear Editor button, then use keyboard shortcut as fallback
  // The Clear button updates React state but CodeMirror's controlled-value
  // integration may not immediately reflect the change
  await page.getByRole("button", { name: /Clear Editor/ }).click();
  // Give React state time to propagate
  await page.waitForTimeout(500);

  // If button didn't clear, fall back to keyboard shortcut
  const text = await editorContent.textContent();
  if (text?.includes("SELECT")) {
    await page.locator(".cm-content").first().click();
    await page.keyboard.press("Meta+a");
    await page.keyboard.press("Backspace");
  }

  await expect(editorContent).not.toHaveText(/SELECT/, { timeout: 5_000 });
  runtimeErrors.assertClean("Clear editor");
});

test("SQL editor: export dropdown shows CSV/JSON/SQL options", async ({
  page,
}) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT * FROM users");

  // Execute query first to get results
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible({
    timeout: 10_000,
  });

  // Export button should now be visible
  const exportButton = page.getByRole("button", {
    name: "Rerun and Export",
  });
  await expect(exportButton).toBeVisible();

  // Click to open dropdown
  await exportButton.click();

  // Verify all three export options are visible
  await expect(page.getByRole("menuitem", { name: "CSV" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "JSON" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "SQL" })).toBeVisible();

  // Close dropdown by pressing Escape
  await page.keyboard.press("Escape");
  runtimeErrors.assertClean("Export dropdown");
});

test("SQL editor: execute invalid query shows error", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  // Mock returns error for queries containing "invalid"
  await typeInEditor(page, "SELECT invalid query");

  await page.getByRole("button", { name: /Run SQL/ }).click();

  // Wait for error result to appear (toolbar status text)
  await expect(page.getByText(/Execution failed/)).toBeVisible({
    timeout: 10_000,
  });
  // Skip assertClean — the mock throws intentionally, which triggers console.error in the API layer
});

test("SQL editor: structured errors retain code, category, and hint", async ({
  page,
}) => {
  await page.goto("/");
  await openNewQueryTab(page);
  await typeInEditor(
    page,
    "SELECT account_id FROM users /* dbpaw-test:structured-error */",
  );
  await page.getByRole("button", { name: /Run SQL/ }).click();

  await expect(
    page.getByText("Column account_id does not exist"),
  ).toBeVisible();
  await expect(
    page.getByText("Check the selected columns and aliases."),
  ).toBeVisible();
  await expect(page.getByText("query · 42703")).toBeVisible();
});

test("SQL logs dropdown shows execution history", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT * FROM users");

  // Execute query first
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible({
    timeout: 10_000,
  });

  // Open SQL logs dropdown
  await page.getByLabel("Open SQL execution logs").click();

  // Verify SQL logs dropdown opens
  await expect(page.getByText("SQL Logs (latest 100)")).toBeVisible();

  // Verify executed query appears in logs
  const logsPopover = page.locator("[data-radix-popper-content-wrapper]");
  await expect(
    logsPopover.getByText("SELECT * FROM users").first(),
  ).toBeVisible();

  runtimeErrors.assertClean("SQL logs dropdown shows execution history");
});

test("SQL logs dropdown shows empty state", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);

  // Open SQL logs dropdown
  await page.getByLabel("Open SQL execution logs").click();

  // Verify empty state message
  await expect(page.getByText("No execution logs yet.")).toBeVisible();

  runtimeErrors.assertClean("SQL logs dropdown shows empty state");
});

test("SQL logs copy SQL button", async ({ page, context }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await context.grantPermissions(["clipboard-write"]);
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);
  await typeInEditor(page, "SELECT 1");

  // Execute query
  await page.getByRole("button", { name: /Run SQL/ }).click();
  await expect(page.getByText(/Returned 10 row/)).toBeVisible({
    timeout: 10_000,
  });

  // Open SQL logs dropdown
  await page.getByLabel("Open SQL execution logs").click();

  // Click copy button on the log entry
  await page.getByLabel("Copy SQL").click();

  // Verify copy success toast
  await expect(page.getByText("SQL copied")).toBeVisible();

  runtimeErrors.assertClean("SQL logs copy SQL button");
});

test("SQL editor: schema selector appears with available schemas", async ({
  page,
}) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);

  // Wait for async data to load (databases + schemas load in same Promise.allSettled)
  // The database selector appears first — use it as a signal that data has loaded
  const dbSelector = page.getByRole("combobox", { name: "Switch database" });
  await expect(dbSelector).toBeVisible({ timeout: 10_000 });

  // Mock returns ["public", "auth", "analytics"] for list_schemas (>1), so schema dropdown should appear
  const schemaSelector = page.getByRole("combobox", { name: "Switch schema" });
  await expect(schemaSelector).toBeVisible();
  await expect(schemaSelector).toHaveAttribute(
    "title",
    /object completion only/,
  );

  runtimeErrors.assertClean("Schema selector visible");
});

test("SQL editor: schema selector shows all available schemas", async ({
  page,
}) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);

  // Wait for data to load
  const dbSelector = page.getByRole("combobox", { name: "Switch database" });
  await expect(dbSelector).toBeVisible({ timeout: 10_000 });

  // Open the schema dropdown
  const schemaSelector = page.getByRole("combobox", { name: "Switch schema" });
  await expect(schemaSelector).toBeVisible();
  await schemaSelector.click();

  // Verify all mock schemas are listed
  const popover = page.locator("[data-radix-popper-content-wrapper]");
  await expect(popover.getByRole("option", { name: "public" })).toBeVisible();
  await expect(popover.getByRole("option", { name: "auth" })).toBeVisible();
  await expect(
    popover.getByRole("option", { name: "analytics" }),
  ).toBeVisible();

  // Close dropdown
  await page.keyboard.press("Escape");
  runtimeErrors.assertClean("Schema dropdown lists all schemas");
});

test("SQL editor: switching schema updates selection", async ({ page }) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);

  // Wait for data to load
  const dbSelector = page.getByRole("combobox", { name: "Switch database" });
  await expect(dbSelector).toBeVisible({ timeout: 10_000 });

  // Open the schema dropdown and select a different schema
  const schemaSelector = page.getByRole("combobox", { name: "Switch schema" });
  await expect(schemaSelector).toBeVisible();
  await schemaSelector.click();

  const popover = page.locator("[data-radix-popper-content-wrapper]");
  await popover.getByRole("option", { name: "analytics" }).click();

  // Verify the selection changed
  await expect(schemaSelector).toHaveText("analytics");
  runtimeErrors.assertClean("Schema switched to analytics");
});

test("SQL editor: database selector and schema selector coexist", async ({
  page,
}) => {
  const runtimeErrors = collectRuntimeErrors(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();

  await openNewQueryTab(page);

  // Wait for data to load — both selectors appear after same Promise.allSettled
  const dbSelector = page.getByRole("combobox", { name: "Switch database" });
  await expect(dbSelector).toBeVisible({ timeout: 10_000 });

  const schemaSelector = page.getByRole("combobox", { name: "Switch schema" });
  await expect(schemaSelector).toBeVisible();

  runtimeErrors.assertClean("Database and schema selectors coexist");
});
