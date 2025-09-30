import { describe } from "vitest";
import { createTestFixtureExtension } from "./fixture/it";

describe("Artifact", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  it("should create an account that hasn't been created", async ({
    expect,
    clients,
  }) => {
    for (const version of ["abc", "0.1.0", "1.1.0"]) {
      await clients.admin.enableArtifactVersion(version).submit();
    }
    const artifacts = await clients.user.listArtifacts().submit();
    const expected = ["1.1.0", "0.1.0", "abc"];
    const versions = artifacts.map((a) => a.version);
    expect(versions).toEqual(expected);

    expect(artifacts[0].builtAt).toEqual(new Date(1758561580000).toISOString());

    // Now delete one and expect there to be 2
    await clients.admin.disableArtifactVersion("0.1.0").submit();
    expect(await clients.user.listArtifacts().submit()).toHaveLength(2);
  });
});
