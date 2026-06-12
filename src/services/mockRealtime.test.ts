import { describe, expect, it, vi } from "vitest";
import { runMockRealtimeSession } from "./mockRealtime";

describe("mock realtime session", () => {
  it("emits partial, final, endpoint, and reply tokens in order", async () => {
    vi.useFakeTimers();
    const events: string[] = [];
    const stop = runMockRealtimeSession("s1", (event) => events.push(event.type));

    await vi.advanceTimersByTimeAsync(2500);
    stop();
    vi.useRealTimers();

    expect(events).toEqual([
      "system.status",
      "transcript.partial",
      "transcript.partial",
      "endpoint.detected",
      "transcript.final",
      "reply.started",
      "reply.token",
      "reply.token",
      "reply.final",
    ]);
  });
});
