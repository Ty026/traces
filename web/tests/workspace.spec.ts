import { test, expect, type BrowserContext } from "@playwright/test";
import { readFile } from "node:fs/promises";
async function login(context: BrowserContext) {
  let token = "";
  for (let i = 0; i < 150; i++) {
    try {
      token = JSON.parse(
        await readFile("test-results/session.json", "utf8"),
      ).token;
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 100));
    }
  }
  if (!token) throw new Error("UI fixture session was not created");
  await context.addCookies([
    {
      name: "traces_session",
      value: token,
      domain: "127.0.0.1",
      path: "/",
      httpOnly: true,
      sameSite: "Lax",
    },
  ]);
}
test("private workspace and static deep links", async ({ page }) => {
  await page.goto("/traces/ui-trace-000");
  await expect(
    page.getByRole("heading", { name: "Every step. The whole story." }),
  ).toBeVisible();
  await expect(page.getByText("Almost ready")).toBeVisible();
  await page.screenshot({ path: "test-results/login.png", fullPage: true });
});
test("filters, pagination, details, payload escaping and return state", async ({
  page,
  context,
}) => {
  await login(context);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/traces");
  await expect(page.locator("tbody tr")).toHaveCount(50);
  await page.screenshot({
    path: "test-results/traces-light.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "Next", exact: true }).click();
  await expect(page.getByText("Page 2", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Previous", exact: true }).click();
  await page.getByRole("textbox", { name: "Search traces" }).fill("Research");
  await page.getByRole("textbox", { name: "Search traces" }).press("Enter");
  await expect(page).toHaveURL(/q=Research/);
  await expect(page.locator("tbody tr")).toHaveCount(32);
  await page.locator(".trace-link").first().click();
  await expect(
    page.getByRole("heading", { name: "Execution", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "search_documents", exact: true })
    .click();
  await expect(
    page.getByText("The search provider timed out. Please retry."),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/detail-light.png",
    fullPage: true,
  });
  await page
    .getByRole("textbox", { name: "Find in this trace" })
    .fill("literal payload");
  await page.getByRole("button", { name: "Find", exact: true }).click();
  await expect(page.getByText("1 matching steps")).toBeVisible();
  await page.locator(".step-select").first().click();
  await expect(
    page.getByText(
      '<img src=x onerror="window.__injected=true"> literal payload',
    ),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { __injected?: boolean }).__injected,
    ),
  ).toBeUndefined();
  await page.getByRole("tab", { name: "Raw JSON" }).click();
  await expect(page.locator(".inspector-body")).toContainText("span_data");
  const download = page.waitForEvent("download");
  await page.getByRole("link", { name: "Export", exact: true }).click();
  expect((await download).suggestedFilename()).toBe("trace.json");
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Execution", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(
      '<img src=x onerror="window.__injected=true"> literal payload',
    ),
  ).toBeVisible();
  await page.getByRole("link", { name: "All traces", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Search traces" }),
  ).toHaveValue("Research");
  await page
    .getByRole("button", { name: "Dark appearance", exact: true })
    .click();
  await page.screenshot({
    path: "test-results/traces-dark.png",
    fullPage: true,
  });
  expect(errors).toEqual([]);
});
test("create, use and revoke an ingestion-only key", async ({
  page,
  context,
  request,
}) => {
  await login(context);
  await page.goto("/keys");
  await page
    .getByRole("button", { name: "Create API key", exact: true })
    .click();
  await page.getByLabel("Name", { exact: true }).fill("Browser test agent");
  await page.getByRole("button", { name: "Create key", exact: true }).click();
  const token = await page.getByTestId("new-key").innerText();
  expect(token).toMatch(/^tr_/);
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(page.locator("body")).not.toContainText(token);
  const result = await request.post("/v1/traces/ingest", {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      data: [
        {
          object: "trace",
          id: "browser-ingest",
          workflow_name: "Browser integration",
        },
      ],
    },
  });
  expect(result.status()).toBe(200);
  expect(
    (
      await request.get("/api/traces", {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).status(),
  ).toBe(401);
  await page.screenshot({
    path: "test-results/keys-light.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await page.getByRole("button", { name: "Revoke key", exact: true }).click();
  await expect(page.getByText("Revoked", { exact: true })).toBeVisible();
  expect(
    (
      await request.post("/v1/traces/ingest", {
        headers: { Authorization: `Bearer ${token}` },
        data: { data: [] },
      })
    ).status(),
  ).toBe(401);
});
test("ten thousand steps stay virtualized and keyboard navigable", async ({
  page,
  context,
}) => {
  await login(context);
  await page.goto("/traces/ui-giant");
  await expect(page.getByText("10,000 steps", { exact: true })).toBeVisible();
  expect(await page.getByRole("treeitem").count()).toBeLessThan(45);
  const tree = page.getByRole("tree", { name: "Execution steps" });
  await tree.evaluate((el) => (el.scrollTop = 400000));
  await expect(page.locator(".tree-row").first()).toHaveCSS("top", /399/);
  expect(await page.getByRole("treeitem").count()).toBeLessThan(45);
  await tree.evaluate((el) => (el.scrollTop = 0));
  await tree.focus();
  await page.keyboard.press("ArrowDown");
  await expect(page).toHaveURL(/span=ui-giant-step-00001/);
  const splitter = page.getByRole("separator", {
    name: "Resize execution panel",
  });
  const width = Number(await splitter.getAttribute("aria-valuenow"));
  await splitter.focus();
  await page.keyboard.press("ArrowRight");
  await expect(splitter).toHaveAttribute("aria-valuenow", String(width + 2));
  const timings = await page.evaluate(async () => {
    const el = document.querySelector(".tree-viewport")!;
    const frames: number[] = [];
    let last = performance.now();
    for (let i = 0; i < 120; i++) {
      el.scrollTop = (i * 3800) % 470000;
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => resolve()),
      );
      const now = performance.now();
      frames.push(now - last);
      last = now;
    }
    return frames.sort((a, b) => a - b);
  });
  const p95 = timings[Math.floor(timings.length * 0.95)];
  console.log(`10k-span scrolling frame p95: ${p95.toFixed(1)} ms`);
  expect(p95).toBeLessThan(80);
  await page.screenshot({ path: "test-results/giant.png", fullPage: true });
});
test("mobile layout and accessible modal dismissal", async ({
  page,
  context,
}) => {
  await login(context);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/traces/ui-trace-001");
  await expect(
    page.getByRole("heading", { name: "Execution", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBeLessThanOrEqual(390);
  await page.screenshot({ path: "test-results/mobile.png", fullPage: true });
  await page.getByRole("button", { name: "Delete trace", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("list scroll survives detail navigation and updates wait for the reader", async ({
  page,
  context,
}) => {
  await login(context);
  await page.goto("/traces");
  await expect(page.locator("tbody tr")).toHaveCount(50);
  const table = page.locator(".trace-table-wrap");
  await table.evaluate((el) => (el.scrollTop = 1200));
  const before = await table.evaluate((el) => el.scrollTop);
  await page.locator(".trace-link").nth(17).click();
  await expect(
    page.getByRole("heading", { name: "Execution", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "All traces", exact: true }).click();
  await expect
    .poll(() => table.evaluate((el) => el.scrollTop))
    .toBeGreaterThanOrEqual(before - 100);
  await table.evaluate((el) => (el.scrollTop = 0));
  const first = await page.locator(".trace-link").first().innerText();
  // A managed key created through the authenticated API sends a new run.
  const key = await page.evaluate(async () => {
    const r = await fetch("/api/keys", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Traces-Request": "1" },
      body: JSON.stringify({ name: "Live refresh test" }),
    });
    return r.json();
  });
  await page.evaluate(async (key: string) => {
    await fetch("/v1/traces/ingest", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${key}`,
      },
      body: JSON.stringify({
        data: [
          {
            object: "trace",
            id: "live-refresh-test",
            workflow_name: "New live arrival",
          },
        ],
      }),
    });
  }, key.key);
  await expect(
    page.getByRole("button", {
      name: "Updated traces available · Show latest",
    }),
  ).toBeVisible({ timeout: 12000 });
  expect(await page.locator(".trace-link").first().innerText()).toBe(first);
  await page
    .getByRole("button", { name: "Updated traces available · Show latest" })
    .click();
  await expect(page.locator(".trace-link").first()).toContainText(
    "New live arrival",
  );
});
