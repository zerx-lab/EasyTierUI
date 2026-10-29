// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root"), as a phone.
import { expect, open, test } from "nsh/test";

test("a phone picks the client mode, opens a page and goes back", async () => {
  const app = await open(".");
  expect(app.texts()).toContain("How will you use EasyTierUI?");
  await app.press("Continue");
  expect(app.texts()).toContain("This device");
  await app.press("id:page-peers");
  expect(app.texts()).toContain("No network is running");
  await app.back();
  expect(app.texts()).not.toContain("No network is running");
});
