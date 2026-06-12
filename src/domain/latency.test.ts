import { describe, expect, it } from "vitest";
import { classifyLatency } from "./latency";

describe("classifyLatency", () => {
  it("marks reply TTFT under 1500 ms as target", () => {
    expect(classifyLatency("reply_ttft", 1200)).toBe("target");
  });

  it("marks reply TTFT above 1500 ms as slow", () => {
    expect(classifyLatency("reply_ttft", 1700)).toBe("slow");
  });

  it("treats a value exactly at the threshold as target", () => {
    expect(classifyLatency("reply_ttft", 1500)).toBe("target");
  });

  it("applies per-metric thresholds", () => {
    expect(classifyLatency("asr_partial", 801)).toBe("slow");
    expect(classifyLatency("endpoint", 500)).toBe("target");
  });
});
