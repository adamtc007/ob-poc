import { describe, expect, it } from "vitest";
import { seedStartKey, uuidV5, SEED_START_NAMESPACE } from "./bpmnTemplates";

// Expected values computed independently with Python:
//   uuid.uuid5(UUID("b13f1a02-d299-4a71-9c3e-7a215c0e8b44"), UUID(sid).bytes)
// mirroring the server's Uuid::new_v5(&NAMESPACE, session_id.as_bytes()).
describe("seedStartKey (RFC-4122 v5 over session id bytes)", () => {
  it("matches the server's seed_start_key for the nil session id", async () => {
    expect(await seedStartKey("00000000-0000-0000-0000-000000000000")).toBe(
      "847dfdfd-9358-5ada-a613-5bde4624cb3d",
    );
  });

  it("matches for a non-trivial session id", async () => {
    expect(await seedStartKey("123e4567-e89b-12d3-a456-426614174000")).toBe(
      "993cff57-8d01-5968-a7d0-6053ead5b5b8",
    );
  });

  it("sets version 5 and RFC variant bits", async () => {
    const out = await uuidV5(
      SEED_START_NAMESPACE,
      new Uint8Array([1, 2, 3, 4]),
    );
    expect(out[14]).toBe("5");
    expect(["8", "9", "a", "b"]).toContain(out[19]);
  });
});
