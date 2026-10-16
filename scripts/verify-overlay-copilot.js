// Open through the Vite server used by dev-app.sh --watch --ui-only; no audio or provider calls.
export default async function verifyOverlayCopilot(page, baseUrl = "http://127.0.0.1:1420") {
  await page.goto(new URL("/scripts/fixtures/overlay-copilot.html", baseUrl).href);
  await page.waitForFunction(() => window.copilotFixtureReady === true);
  await page.waitForSelector('[data-testid="toggle-copilot"][aria-pressed="false"]');
  if (await page.evaluate(() => Boolean(document.querySelector('[data-testid="copilot-pane"]')))) throw new Error("inactive pane visible");
  await page.evaluate(() => window.renderCopilotFixture(true));
  await page.waitForSelector('[data-testid="toggle-copilot"][aria-pressed="true"]');
  await page.waitForSelector('[data-testid="copilot-pane"]');
  const labels = await page.evaluate(() => [...document.querySelectorAll(".copilot-speaker")].map(node => node.textContent));
  if (labels.join(",") !== "Me,Other") throw new Error(`speaker labels: ${labels.join(",")}`);
  await page.evaluate(() => window.renderCopilotFixture(false));
  await page.waitForSelector('[data-testid="toggle-copilot"][aria-pressed="false"]');
  if (await page.evaluate(() => Boolean(document.querySelector('[data-testid="copilot-pane"]')))) throw new Error("closed pane retained");
  return { states: 2, labels };
}
