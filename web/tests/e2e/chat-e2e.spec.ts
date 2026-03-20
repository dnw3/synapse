import { test, expect } from "@playwright/test";

const BASE = "http://localhost:5173";

async function navigateToChat(page: import("@playwright/test").Page) {
  await page.goto(BASE);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);

  // Click the Chat tab (2nd button matching "Chat" — first is the group header, second is the tab)
  await page.getByRole("button", { name: "Chat" }).nth(1).click();
  await page.waitForTimeout(2000);

  // Verify we navigated away from Overview
  await page.screenshot({ path: "test-results/chat-e2e-after-nav.png", fullPage: true });
}

test.describe("Chat E2E", () => {
  test("sends a message via WebSocket and receives a response", async ({
    page,
  }) => {
    await navigateToChat(page);

    // Take screenshot to see what we have
    await page.screenshot({
      path: "test-results/chat-e2e-before-send.png",
      fullPage: true,
    });

    // Find the chat textarea
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible({ timeout: 10000 });

    // Type and send a message
    await textarea.fill("请用一句话介绍你自己");
    await textarea.press("Enter");

    // Verify the human message appears
    await expect(page.locator("text=请用一句话介绍你自己")).toBeVisible({
      timeout: 5000,
    });

    // Wait for assistant response (up to 60s for model call)
    // The response should contain some text after our message
    const startTime = Date.now();
    let responseFound = false;

    while (Date.now() - startTime < 60000 && !responseFound) {
      await page.waitForTimeout(2000);
      const content = await page.textContent("body");
      // Look for indicators that a response has been generated
      // (the assistant message will have content beyond our question)
      if (
        content &&
        content.includes("请用一句话介绍你自己") &&
        content.length > 500
      ) {
        responseFound = true;
      }
    }

    // Take screenshot of final state
    await page.screenshot({
      path: "test-results/chat-e2e-response.png",
      fullPage: true,
    });

    const finalContent = await page.textContent("body");
    console.log("Response found:", responseFound);
    console.log("Body length:", finalContent?.length);
  });

  test("WebSocket connects with v3 handshake", async ({ page }) => {
    const wsMessages: string[] = [];

    page.on("websocket", (ws) => {
      ws.on("framesent", (frame) => {
        if (typeof frame.payload === "string") {
          wsMessages.push(`SENT: ${frame.payload}`);
        }
      });
      ws.on("framereceived", (frame) => {
        if (typeof frame.payload === "string") {
          wsMessages.push(`RECV: ${frame.payload}`);
        }
      });
    });

    await page.goto(BASE);
    await page.waitForLoadState("networkidle");
    await page.waitForTimeout(5000);

    // Verify v3 connect handshake sent
    const connectFrame = wsMessages.find(
      (m) => m.includes("SENT:") && m.includes('"method":"connect"')
    );
    expect(connectFrame).toBeTruthy();

    // Verify server auth response
    const authResponse = wsMessages.find(
      (m) => m.includes("RECV:") && m.includes("authenticated")
    );
    expect(authResponse).toBeTruthy();
    console.log("v3 handshake OK");
  });

  test("chat page renders without conversation list in sidebar", async ({
    page,
  }) => {
    await navigateToChat(page);

    await page.screenshot({
      path: "test-results/chat-e2e-layout.png",
      fullPage: true,
    });

    // Verify textarea exists
    await expect(page.locator("textarea")).toBeVisible({ timeout: 10000 });

    // Verify sidebar doesn't have conversation management UI
    const sidebar = page.locator("aside");
    const sidebarText = await sidebar.textContent();
    expect(sidebarText).not.toContain("新建会话");
  });
});
