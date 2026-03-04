import { describe, expect, test } from "bun:test";
import { invokeMock } from "./mocks";

describe("invokeMock service layer", () => {
  test("returns table list for metadata command", async () => {
    const tables = await invokeMock<{ schema: string; name: string; type: string }[]>(
      "list_tables",
      {
        id: 1,
        database: "test_db",
        schema: "public",
      },
    );

    expect(tables.length).toBeGreaterThan(0);
    expect(tables[0]).toHaveProperty("schema");
    expect(tables[0]).toHaveProperty("name");
    expect(tables[0]).toHaveProperty("type");
  });

  test("returns connection list for connection command", async () => {
    const connections = await invokeMock<any[]>("get_connections");
    expect(connections.length).toBeGreaterThan(0);
  });

  test("throws on unknown command", async () => {
    expect(invokeMock("unknown_command_for_test")).rejects.toThrow(
      "Mock: Unknown command",
    );
  });
});
