import { expect, test } from "@playwright/test";
import { collectRuntimeErrors } from "./runtime-errors";

test("proxy settings validate, save and reopen with credentials redacted", async ({
  page,
}) => {
  const errors = collectRuntimeErrors(page);
  await page.goto("/");
  const edit = async () => {
    await page
      .getByText("PostgreSQL Dev", { exact: true })
      .click({ button: "right" });
    await page
      .locator(".fixed.z-50")
      .getByRole("button", { name: "Edit", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toBeVisible();
  };
  await edit();
  await expect(
    page.getByText("Use network proxy", { exact: true }),
  ).toHaveCount(1);
  await page.getByLabel("Use network proxy", { exact: true }).click();
  await page.getByLabel("Proxy host", { exact: true }).fill("127.0.0.1");
  await page.getByLabel("Proxy port", { exact: true }).fill("70000");
  const save = page.getByRole("button", { name: "Save", exact: true });
  await expect(save).toBeDisabled();
  await page.getByLabel("Proxy port", { exact: true }).fill("7890");
  await page
    .getByLabel("Proxy username (optional)", { exact: true })
    .fill("proxy-user");
  await page
    .getByLabel("Proxy password (optional)", { exact: true })
    .fill("proxy-secret");
  await page.getByLabel("Proxy type", { exact: true }).click();
  await page.getByRole("option", { name: "HTTP CONNECT", exact: true }).click();
  await expect(save).toBeEnabled();
  await save.click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await edit();
  await expect(
    page.getByLabel("Use network proxy", { exact: true }),
  ).toBeChecked();
  await expect(page.getByLabel("Proxy port", { exact: true })).toHaveValue(
    "7890",
  );
  await expect(page.getByLabel("Proxy type", { exact: true })).toHaveText(
    "HTTP CONNECT",
  );
  await expect(
    page.getByLabel("Proxy username (optional)", { exact: true }),
  ).toHaveValue("proxy-user");
  await expect(
    page.getByLabel("Proxy password (optional)", { exact: true }),
  ).toHaveValue("");
  await page.getByLabel("Use network proxy", { exact: true }).click();
  await expect(page.getByLabel("Proxy host", { exact: true })).toBeHidden();
  await save.click();
  await expect(page.getByRole("dialog")).toBeHidden();
  errors.assertClean("network proxy settings");
});
