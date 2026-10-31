import { expect, test, type Page } from "@playwright/test";
import { collectRuntimeErrors } from "./runtime-errors";

async function openTable(page: Page) {
  await expect(
    page.getByRole("heading", { name: "Connections" }),
  ).toBeVisible();
  // Double-click PostgreSQL Dev to connect
  await page.getByText("PostgreSQL Dev", { exact: true }).dblclick();
  // Wait for connection to establish and child nodes to appear
  await expect(page.getByText("testdb")).toBeVisible();
  // Click testdb to expand it and show schemas
  await page.getByText("testdb").click();
  // Expand public schema
  await expect(page.getByText("public")).toBeVisible();
  await page.getByText("public").click();
  // Expand Tables group
  await expect(page.getByText("Tables")).toBeVisible();
  await page.getByText("Tables").click();
  // Double-click users table to open
  await page.getByText("users", { exact: true }).dblclick();
  // Wait for table data to render
  await expect(page.getByText("alice", { exact: true })).toBeVisible();
  // Inline editing is enabled only after table metadata (including PKs)
  // arrives, which is independent from the table-data request.
  await expect(
    page.locator('[data-row-index="0"][data-col-index="1"]'),
  ).toHaveClass(/cursor-pointer/);
  await expect(page.getByLabel("Add row")).toBeEnabled();
}

test.describe("DataGrid", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await openTable(page);
  });

  test("table view should load without runtime errors", async ({ page }) => {
    const runtimeErrors = collectRuntimeErrors(page);
    await expect(page.getByText("alice", { exact: true })).toBeVisible();
    await expect(page.getByText("bob", { exact: true })).toBeVisible();

    // Scope to the DataGrid tab panel (sidebar also has Refresh button)
    const tabPanel = page.locator("[role='tabpanel']").filter({
      has: page.getByText("alice", { exact: true }),
    });
    await expect(tabPanel.getByLabel("Refresh")).toBeVisible();
    await expect(tabPanel.getByLabel("Search")).toBeVisible();
    await expect(tabPanel.getByLabel("Export")).toBeVisible();

    runtimeErrors.assertClean("Table view should not emit runtime errors");
  });

  test("all toolbar buttons exist", async ({ page }) => {
    const runtimeErrors = collectRuntimeErrors(page);

    // Scope to the DataGrid tab panel (sidebar also has buttons)
    const tabPanel = page.locator("[role='tabpanel']").filter({
      has: page.getByText("alice", { exact: true }),
    });

    // Pagination controls
    await expect(tabPanel.getByLabel("Previous page")).toBeVisible();
    await expect(tabPanel.getByLabel("Next page")).toBeVisible();

    // Core toolbar buttons
    await expect(tabPanel.getByLabel("Refresh")).toBeVisible();
    await expect(tabPanel.getByLabel("Search")).toBeVisible();
    await expect(tabPanel.getByLabel("Export")).toBeVisible();

    // DDL, ER Diagram, New Query buttons
    await expect(tabPanel.getByLabel("DDL")).toBeVisible();
    await expect(tabPanel.getByLabel("ER Diagram")).toBeVisible();
    await expect(
      tabPanel.getByLabel("New Query", { exact: false }),
    ).toBeVisible();

    runtimeErrors.assertClean("Toolbar buttons should not emit runtime errors");
  });

  test("click refresh/search/DDL buttons without errors", async ({ page }) => {
    const runtimeErrors = collectRuntimeErrors(page);

    // Scope to the DataGrid tab panel (sidebar also has Refresh button)
    const tabPanel = page.locator("[role='tabpanel']").filter({
      has: page.getByText("alice", { exact: true }),
    });

    // Click Refresh
    await tabPanel.getByLabel("Refresh").click();
    await page.waitForTimeout(300);

    // Click Search to open it
    await tabPanel.getByLabel("Search").click();
    await expect(
      page.locator('input[placeholder="Search keyword..."]'),
    ).toBeVisible();
    // Close search
    await page.keyboard.press("Escape");

    // Click DDL
    await tabPanel.getByLabel("DDL").click();
    await page.waitForTimeout(300);

    runtimeErrors.assertClean(
      "Refresh/Search/DDL buttons should not emit runtime errors",
    );
  });

  test.describe("Pagination", () => {
    test("navigate to next and previous page", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Next page button
      await page.getByLabel("Next page").click();
      // Verify page input changes to 2
      const pageInput = page.locator('input[inputmode="numeric"]');
      await expect(pageInput).toHaveValue("2");
      // Verify user_101 is visible (page 2 starts at row 101 with limit 100)
      await expect(page.getByText("user_101", { exact: true })).toBeVisible();

      // Click Previous page button
      await page.getByLabel("Previous page").click();
      // Verify back to page 1
      await expect(pageInput).toHaveValue("1");
      // Verify alice is visible again
      await expect(page.getByText("alice", { exact: true })).toBeVisible();

      runtimeErrors.assertClean("Pagination should not emit runtime errors");
    });

    test("jump to page by input", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      const pageInput = page.locator('input[inputmode="numeric"]');
      // Fill page input with "3" and press Enter
      await pageInput.fill("3");
      await pageInput.press("Enter");
      // Verify user_201 is visible (page 3 starts at row 201 with limit 100)
      await expect(page.getByText("user_201", { exact: true })).toBeVisible();

      runtimeErrors.assertClean(
        "Page jump by input should not emit runtime errors",
      );
    });

    test("change page size", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click the page size select trigger
      await page.locator('[role="combobox"]').click();
      // Choose option "50"
      await page.getByRole("option", { name: "50", exact: true }).click();
      // Verify page count appears (could be exact number or "?" when lazy total is off)
      await expect(page.getByText(/\/ (\d+|\?)$/)).toBeVisible();

      runtimeErrors.assertClean(
        "Page size change should not emit runtime errors",
      );
    });
  });

  test.describe("Search", () => {
    test("find fields, navigate highlights, and clear without filtering rows", async ({
      page,
    }) => {
      const runtimeErrors = collectRuntimeErrors(page);
      await page.setViewportSize({ width: 800, height: 720 });
      await page
        .getByRole("button", { name: "Find field", exact: true })
        .click();
      const input = page.getByRole("textbox", {
        name: "Field name or comment...",
      });
      await input.fill("_AT");
      await expect(page.locator('[data-field-match="true"]')).toHaveCount(2);
      await expect(page.locator('[data-field-active="true"]')).toHaveText(
        "created_at",
      );
      await expect(
        page.getByRole("status").filter({ hasText: "Field 1 of 2" }),
      ).toBeVisible();
      await input.press("Enter");
      const activeField = page.locator('[data-field-active="true"]');
      await expect(activeField).toHaveText("updated_at");
      const isInScrollViewport = await activeField.evaluate((element) => {
        const container = element.closest("div.overflow-auto")!;
        const bounds = container.getBoundingClientRect();
        const field = element.getBoundingClientRect();
        return field.left >= bounds.left - 1 && field.right <= bounds.right + 1;
      });
      expect(isInScrollViewport).toBe(true);
      await input.press("Shift+Enter");
      await expect(activeField).toHaveText("created_at");
      await page
        .getByRole("button", { name: "Previous matching field" })
        .click();
      await expect(activeField).toHaveText("updated_at");
      await page.getByRole("button", { name: "Next matching field" }).click();
      await expect(activeField).toHaveText("created_at");
      await input.fill("Email address");
      await expect(activeField).toHaveText("email");
      await input.fill("no_such_field");
      await expect(
        page.getByText("No matching fields", { exact: true }),
      ).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Next matching field" }),
      ).toBeDisabled();
      await page.getByRole("button", { name: "Clear field search" }).click();
      await expect(page.locator('[data-field-match="true"]')).toHaveCount(0);
      await page.keyboard.press("Escape");
      await expect(page.getByText("alice", { exact: true })).toBeVisible();
      runtimeErrors.assertClean("Field search should not emit runtime errors");
    });

    test("retains field highlighting on refresh and when switching views", async ({
      page,
    }) => {
      const runtimeErrors = collectRuntimeErrors(page);
      await page
        .getByRole("button", { name: "Find field", exact: true })
        .click();
      await page
        .getByRole("textbox", { name: "Field name or comment..." })
        .fill("updated_at");
      await page.keyboard.press("Escape");
      const activeField = page.locator('[data-field-active="true"]');
      await expect(activeField).toHaveText("updated_at");
      const panel = page
        .getByRole("tabpanel")
        .filter({ has: page.getByText("alice", { exact: true }) });
      await panel.getByLabel("Refresh", { exact: true }).click();
      await expect(activeField).toHaveText("updated_at");
      await panel.getByLabel("Toggle column view").click();
      await expect(page.locator('td[data-field-active="true"]')).toHaveText(
        "updated_at",
      );
      await panel.getByLabel("Toggle table view").click();
      await expect(page.locator('th[data-field-active="true"]')).toHaveText(
        "updated_at",
      );
      // Existing cell-content search continues to work independently.
      await panel.getByLabel("Search", { exact: true }).click();
      await page.getByPlaceholder("Search keyword...").fill("alice");
      await expect(page.getByText(/row\(s\).*match\(es\)/)).toBeVisible();
      await expect(activeField).toHaveText("updated_at");
      runtimeErrors.assertClean(
        "Field search should survive refresh and view changes",
      );
    });

    test("open search and show match count", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Search button
      await page.getByLabel("Search").click();
      // Fill search input with "alice"
      await page
        .locator('input[placeholder="Search keyword..."]')
        .fill("alice");
      await page
        .locator('input[placeholder="Search keyword..."]')
        .press("Enter");
      // Verify row(s)/match(es) text is visible
      await expect(page.getByText(/row\(s\).*match\(es\)/)).toBeVisible();

      runtimeErrors.assertClean("Search should not emit runtime errors");
    });

    test("cycle through matches", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Search button
      await page.getByLabel("Search").click();
      // Fill search input with "user_1" and press Enter
      await page
        .locator('input[placeholder="Search keyword..."]')
        .fill("user_1");
      await page
        .locator('input[placeholder="Search keyword..."]')
        .press("Enter");
      // Verify match text is visible
      await expect(page.getByText(/match\(es\)/)).toBeVisible();

      runtimeErrors.assertClean(
        "Search cycling should not emit runtime errors",
      );
    });

    test("close search with Escape", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Open search
      await page.getByLabel("Search").click();
      await expect(
        page.locator('input[placeholder="Search keyword..."]'),
      ).toBeVisible();

      // Press Escape to close
      await page.keyboard.press("Escape");
      // Verify search input is hidden
      await expect(
        page.locator('input[placeholder="Search keyword..."]'),
      ).toBeHidden();

      runtimeErrors.assertClean("Search close should not emit runtime errors");
    });
  });

  test.describe("View Switch", () => {
    test("toggle between table and column view", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click "Toggle column view"
      await page.getByLabel("Toggle column view").click();
      // Verify we're in column view by checking the toggle button changed
      await expect(page.getByLabel("Toggle table view")).toBeVisible();

      // Click "Toggle table view" to switch back
      await page.getByLabel("Toggle table view").click();
      // Verify alice is still visible in table view
      await expect(page.getByText("alice", { exact: true })).toBeVisible();

      runtimeErrors.assertClean("View switch should not emit runtime errors");
    });
  });

  test.describe("Filter & Sort", () => {
    test("apply WHERE filter", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Fill WHERE input
      const whereInput = page.locator('input[placeholder="WHERE ..."]');
      await whereInput.fill("username = 'alice'");
      await whereInput.press("Enter");
      // Verify alice is visible
      await expect(page.getByText("alice", { exact: true })).toBeVisible();

      runtimeErrors.assertClean("WHERE filter should not emit runtime errors");
    });

    test("apply ORDER BY", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Fill ORDER BY input
      const orderByInput = page.locator('input[placeholder="ORDER BY ..."]');
      await orderByInput.fill("id DESC");
      await orderByInput.press("Enter");
      // Verify data is visible (first row should be user_250 or similar)
      await expect(page.locator("tbody tr").first()).toBeVisible();

      runtimeErrors.assertClean("ORDER BY should not emit runtime errors");
    });

    test("sort by clicking column header", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click the "username" th element
      await page.locator("th", { hasText: "username" }).click();
      // Verify data is still visible
      await expect(page.locator("tbody tr").first()).toBeVisible();

      runtimeErrors.assertClean(
        "Column header sort should not emit runtime errors",
      );
    });
  });

  test.describe("DDL & ER Diagram", () => {
    test("open DDL", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click DDL button
      await page.getByLabel("DDL").click();
      // Wait for DDL to render
      await page.waitForTimeout(500);
      runtimeErrors.assertClean("DDL should not emit runtime errors");
    });

    test("open ER diagram", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click ER Diagram button
      await page.getByLabel("ER Diagram").click();
      // Wait for ER diagram to render
      await page.waitForTimeout(500);
      runtimeErrors.assertClean("ER diagram should not emit runtime errors");
    });
  });

  test.describe("Row Mutation", () => {
    test("add draft row", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Add row button
      await page.getByLabel("Add row").click();
      // Verify Save and Discard buttons are visible
      await expect(page.getByLabel("Save")).toBeVisible();
      await expect(page.getByLabel("Discard")).toBeVisible();

      runtimeErrors.assertClean("Add draft row should not emit runtime errors");
    });

    test("discard changes", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Add a row first
      await page.getByLabel("Add row").click();
      await expect(page.getByLabel("Save")).toBeVisible();

      // Click Discard button
      await page.getByLabel("Discard").click();
      // Verify Save button is hidden
      await expect(page.getByLabel("Save")).toBeHidden();

      runtimeErrors.assertClean(
        "Discard changes should not emit runtime errors",
      );
    });

    test("delete button disabled without selection", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Verify Delete selected rows button is disabled
      await expect(page.getByLabel("Delete selected rows")).toBeDisabled();

      runtimeErrors.assertClean(
        "Delete button state should not emit runtime errors",
      );
    });
  });

  test.describe("Cell Editing", () => {
    test("double-click cell to enter edit mode", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Double-click the username cell of the first row (alice)
      const cell = page.locator('[data-row-index="0"][data-col-index="1"]');
      await cell.dblclick();

      // Verify input appears with current value
      const input = cell.locator("input");
      await expect(input).toBeVisible();
      await expect(input).toHaveValue("alice");

      runtimeErrors.assertClean(
        "Double-click edit should not emit runtime errors",
      );
    });

    test("commit edit with Enter", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Double-click the username cell of the first row
      const cell = page.locator('[data-row-index="0"][data-col-index="1"]');
      await cell.dblclick();

      // Clear and type new value, then press Enter
      const input = cell.locator("input");
      await input.clear();
      await input.fill("edited_name");
      await input.press("Enter");

      // Verify cell shows new value
      await expect(cell).toContainText("edited_name");

      // Verify modified indicator (orange left border)
      await expect(cell).toHaveClass(/border-l-orange-400/);

      // Verify Save button appears
      await expect(page.getByLabel("Save")).toBeVisible();

      runtimeErrors.assertClean(
        "Commit edit with Enter should not emit runtime errors",
      );
    });

    test("cancel edit with Escape", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Double-click the username cell of the first row
      const cell = page.locator('[data-row-index="0"][data-col-index="1"]');
      await cell.dblclick();

      // Type something and press Escape
      const input = cell.locator("input");
      await input.clear();
      await input.fill("should_not_persist");
      await input.press("Escape");

      // Verify original value is preserved
      await expect(cell).toContainText("alice");

      // Verify Save button is not visible
      await expect(page.getByLabel("Save")).toBeHidden();

      runtimeErrors.assertClean(
        "Cancel edit with Escape should not emit runtime errors",
      );
    });

    test("commit edit on blur (click away)", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Double-click the username cell of the first row
      const firstCell = page.locator(
        '[data-row-index="0"][data-col-index="1"]',
      );
      await firstCell.dblclick();

      // Type new value
      const input = firstCell.locator("input");
      await input.clear();
      await input.fill("blurred");

      // Click another cell to blur
      const secondCell = page.locator(
        '[data-row-index="1"][data-col-index="1"]',
      );
      await secondCell.click();

      // Verify first cell shows edited value
      await expect(firstCell).toContainText("blurred");

      runtimeErrors.assertClean(
        "Commit edit on blur should not emit runtime errors",
      );
    });

    test("save and discard changes", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Edit a cell
      const cell = page.locator('[data-row-index="0"][data-col-index="1"]');
      await cell.dblclick();
      const input = cell.locator("input");
      await input.clear();
      await input.fill("saved_value");
      await input.press("Enter");

      // Save the change
      await expect(page.getByLabel("Save")).toBeVisible();
      await page.getByLabel("Save").click();

      // Verify Save button disappears after save
      await expect(page.getByLabel("Save")).toBeHidden();

      // Edit again to test discard
      await cell.dblclick();
      const input2 = cell.locator("input");
      await input2.clear();
      await input2.fill("will_be_discarded");
      await input2.press("Enter");

      // Discard the change
      await expect(page.getByLabel("Discard")).toBeVisible();
      await page.getByLabel("Discard").click();

      // Verify original value is restored
      await expect(cell).toContainText("alice");

      runtimeErrors.assertClean(
        "Save and discard should not emit runtime errors",
      );
    });

    test("Tab key commits edit", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Double-click the username cell of the first row (col-index 1)
      const usernameCell = page.locator(
        '[data-row-index="0"][data-col-index="1"]',
      );
      await usernameCell.dblclick();

      // Edit value and press Tab
      const input = usernameCell.locator("input");
      await input.clear();
      await input.fill("tab_edited");
      await input.press("Tab");

      // Verify edit was committed
      await expect(usernameCell).toContainText("tab_edited");
      await expect(usernameCell).toHaveClass(/border-l-orange-400/);

      runtimeErrors.assertClean(
        "Tab key commit should not emit runtime errors",
      );
    });

    test("multi-cell edit shows all pending changes", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Edit first cell (username)
      const usernameCell = page.locator(
        '[data-row-index="0"][data-col-index="1"]',
      );
      await usernameCell.dblclick();
      const usernameInput = usernameCell.locator("input");
      await usernameInput.clear();
      await usernameInput.fill("new_alice");
      await usernameInput.press("Enter");

      // Edit second cell (email)
      const emailCell = page.locator(
        '[data-row-index="0"][data-col-index="2"]',
      );
      await emailCell.dblclick();
      const emailInput = emailCell.locator("input");
      await emailInput.clear();
      await emailInput.fill("new@example.com");
      await emailInput.press("Enter");

      // Verify both cells show modified indicator
      await expect(usernameCell).toHaveClass(/border-l-orange-400/);
      await expect(emailCell).toHaveClass(/border-l-orange-400/);

      // Verify Save button is visible
      await expect(page.getByLabel("Save")).toBeVisible();

      runtimeErrors.assertClean(
        "Multi-cell edit should not emit runtime errors",
      );
    });

    test("draft row add, fill, and save", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Add row button
      await page.getByLabel("Add row").click();

      // Draft rows are appended after the virtualized data rows.
      await page
        .locator("table")
        .last()
        .evaluate((table) => {
          const scroller = table.parentElement;
          if (!scroller) return;
          scroller.scrollTop = scroller.scrollHeight;
          scroller.dispatchEvent(new Event("scroll"));
        });

      // Verify draft row appears with input fields (draft rows have data-draft-id)
      const draftInput = page.locator('input[data-draft-col-index="0"]');
      await expect(draftInput).toBeVisible();

      // Fill id column (first input in draft row)
      await draftInput.fill("999");

      // Fill username column (second input)
      const usernameInput = page.locator('input[data-draft-col-index="1"]');
      await usernameInput.fill("draft_user");
      await page
        .locator('input[data-draft-col-index="2"]')
        .fill("draft@example.com");
      await page
        .locator('input[data-draft-col-index="3"]')
        .fill("hashed_password");

      // Verify Save button is visible
      await expect(page.getByLabel("Save")).toBeVisible();

      // Save the changes
      await page.getByLabel("Save").click();

      // Verify Save button disappears
      await expect(page.getByLabel("Save")).toBeHidden();

      runtimeErrors.assertClean(
        "Draft row add and save should not emit runtime errors",
      );
    });

    test("edit with WHERE filter applied", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Apply WHERE filter
      const whereInput = page.locator('input[placeholder="WHERE ..."]');
      await whereInput.fill("username = 'alice'");
      await whereInput.press("Enter");

      // Wait for filtered data
      await expect(page.getByText("alice", { exact: true })).toBeVisible();

      // Edit alice's email cell
      const emailCell = page.locator(
        '[data-row-index="0"][data-col-index="2"]',
      );
      await emailCell.dblclick();
      const input = emailCell.locator("input");
      await input.clear();
      await input.fill("filtered_edit@example.com");
      await input.press("Enter");

      // Verify edit was applied
      await expect(emailCell).toContainText("filtered_edit@example.com");
      await expect(emailCell).toHaveClass(/border-l-orange-400/);

      // Verify Save button appears
      await expect(page.getByLabel("Save")).toBeVisible();

      runtimeErrors.assertClean(
        "Edit with filter should not emit runtime errors",
      );
    });

    test("edit cell to NULL value", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Edit email cell to empty (NULL)
      const emailCell = page.locator(
        '[data-row-index="0"][data-col-index="2"]',
      );
      await emailCell.dblclick();
      const input = emailCell.locator("input");
      await input.clear();
      await input.press("Enter");

      // Verify cell shows NULL or empty
      await expect(emailCell).toHaveClass(/border-l-orange-400/);

      // Verify Save button appears
      await expect(page.getByLabel("Save")).toBeVisible();

      runtimeErrors.assertClean(
        "NULL value edit should not emit runtime errors",
      );
    });

    test("Ctrl+S triggers save when changes pending", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Edit a cell
      const cell = page.locator('[data-row-index="0"][data-col-index="1"]');
      await cell.dblclick();
      const input = cell.locator("input");
      await input.clear();
      await input.fill("ctrl_s_test");
      await input.press("Enter");

      // Verify Save button is visible
      await expect(page.getByLabel("Save")).toBeVisible();

      // Press Ctrl+S to save
      await page.keyboard.press("Control+s");

      // Verify Save button disappears after save
      await expect(page.getByLabel("Save")).toBeHidden();

      runtimeErrors.assertClean("Ctrl+S save should not emit runtime errors");
    });
  });

  test.describe("Export", () => {
    test("open export menu", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Export button
      await page.getByLabel("Export").click();
      // Verify export menu items are visible
      await expect(
        page.getByRole("menuitem", { name: "Export Current Page" }),
      ).toBeVisible();
      await expect(
        page.getByRole("menuitem", { name: "Export Filtered Result" }),
      ).toBeVisible();
      await expect(
        page.getByRole("menuitem", { name: "Export Full Table" }),
      ).toBeVisible();

      runtimeErrors.assertClean("Export menu should not emit runtime errors");
    });

    test("show format options", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);

      // Click Export button
      await page.getByLabel("Export").click();
      // Hover "Export Current Page" to show format options
      await page.getByRole("menuitem", { name: "Export Current Page" }).hover();
      // Verify CSV/JSON/SQL menu items are visible
      await expect(page.getByRole("menuitem", { name: "CSV" })).toBeVisible();
      await expect(page.getByRole("menuitem", { name: "JSON" })).toBeVisible();
      await expect(page.getByRole("menuitem", { name: "SQL" })).toBeVisible();

      runtimeErrors.assertClean(
        "Export format options should not emit runtime errors",
      );
    });
  });

  test.describe("Export Execution", () => {
    async function exportCurrentPage(page: Page, format: string) {
      await page.getByLabel("Export").click();
      await page.getByRole("menuitem", { name: "Export Current Page" }).hover();
      await page.getByRole("menuitem", { name: format }).click();
      await expect(
        page.getByText(/Export dialog is only available/),
      ).toBeVisible();
    }

    test("export current page to CSV", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);
      await exportCurrentPage(page, "CSV");
      runtimeErrors.assertClean(
        "Export current page to CSV should not emit runtime errors",
      );
    });

    test("export current page to JSON", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);
      await exportCurrentPage(page, "JSON");
      runtimeErrors.assertClean(
        "Export current page to JSON should not emit runtime errors",
      );
    });

    test("export current page to SQL", async ({ page }) => {
      const runtimeErrors = collectRuntimeErrors(page);
      await exportCurrentPage(page, "SQL");
      runtimeErrors.assertClean(
        "Export current page to SQL should not emit runtime errors",
      );
    });
  });
});
