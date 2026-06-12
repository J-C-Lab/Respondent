import { describe, expect, it } from "vitest";
import { createReplyEngine } from "./replyEngine";

describe("reply engine", () => {
  it("does not trigger from partial transcript alone", () => {
    const engine = createReplyEngine("s1");
    const action = engine.apply({
      type: "transcript.partial",
      sessionId: "s1",
      text: "Can you explain",
      startedAtMs: 0,
      endedAtMs: 400,
      receivedAtMs: 450,
    });
    expect(action).toEqual({ type: "none" });
  });

  it("triggers after endpoint and final transcript", () => {
    const engine = createReplyEngine("s1");
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 1200,
    });
    const action = engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "Can you explain the timeline?",
      startedAtMs: 0,
      endedAtMs: 1100,
      receivedAtMs: 1300,
    });
    expect(action).toEqual({
      type: "start-reply",
      transcript: "Can you explain the timeline?",
      context: ["Can you explain the timeline?"],
    });
  });

  it("does not arm a local reply from a foreign endpoint", () => {
    const engine = createReplyEngine("s1");
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s2",
      silenceMs: 300,
      detectedAtMs: 1200,
    });

    const action = engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "Can you explain the timeline?",
      startedAtMs: 0,
      endedAtMs: 1100,
      receivedAtMs: 1300,
    });

    expect(action).toEqual({ type: "none" });
  });

  it("does not add foreign final transcripts to local context", () => {
    const engine = createReplyEngine("s1");
    engine.apply({
      type: "transcript.final",
      sessionId: "s2",
      text: "Use the other session",
      startedAtMs: 0,
      endedAtMs: 1100,
      receivedAtMs: 1300,
    });

    expect(engine.context()).toEqual([]);
  });

  it("returns a start-reply context copy that cannot mutate internal context", () => {
    const engine = createReplyEngine("s1");
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 1200,
    });
    const action = engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "Can you explain the timeline?",
      startedAtMs: 0,
      endedAtMs: 1100,
      receivedAtMs: 1300,
    });

    expect(action.type).toBe("start-reply");
    if (action.type !== "start-reply") return;
    action.context.push("mutated outside");

    expect(engine.context()).toEqual(["Can you explain the timeline?"]);
  });

  it("triggers at most one reply for a single endpoint", () => {
    const engine = createReplyEngine("s1");
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 1200,
    });

    expect(
      engine.apply({
        type: "transcript.final",
        sessionId: "s1",
        text: "First final",
        startedAtMs: 0,
        endedAtMs: 1100,
        receivedAtMs: 1300,
      }),
    ).toEqual({
      type: "start-reply",
      transcript: "First final",
      context: ["First final"],
    });
    expect(
      engine.apply({
        type: "transcript.final",
        sessionId: "s1",
        text: "Second final",
        startedAtMs: 1400,
        endedAtMs: 1800,
        receivedAtMs: 1900,
      }),
    ).toEqual({ type: "none" });
  });

  it("does not trigger a stale reply when the matching final never arrives", () => {
    const engine = createReplyEngine("s1", { staleArmMs: 2000 });
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 1000,
    });

    // The matching final is lost. A much later final from a new turn arrives.
    const action = engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "A totally different later sentence.",
      startedAtMs: 8000,
      endedAtMs: 8400,
      receivedAtMs: 8500,
    });

    expect(action).toEqual({ type: "none" });
    expect(engine.context()).toEqual(["A totally different later sentence."]);
  });

  it("re-arms normally after a stale endpoint is discarded", () => {
    const engine = createReplyEngine("s1", { staleArmMs: 2000 });
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 1000,
    });
    // Orphaned: matching final lost, late final discards the stale arm.
    engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "late orphan",
      startedAtMs: 8000,
      endedAtMs: 8400,
      receivedAtMs: 8500,
    });

    // A fresh endpoint + final pair must still trigger.
    engine.apply({
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 9000,
    });
    const action = engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "fresh turn",
      startedAtMs: 8900,
      endedAtMs: 9050,
      receivedAtMs: 9200,
    });

    expect(action).toEqual({
      type: "start-reply",
      transcript: "fresh turn",
      context: ["late orphan", "fresh turn"],
    });
  });

  it("does not trigger when a final arrives before any endpoint", () => {
    const engine = createReplyEngine("s1");
    const action = engine.apply({
      type: "transcript.final",
      sessionId: "s1",
      text: "no endpoint yet",
      startedAtMs: 0,
      endedAtMs: 400,
      receivedAtMs: 500,
    });
    expect(action).toEqual({ type: "none" });
  });

  it("keeps only the latest six final turns in context", () => {
    const engine = createReplyEngine("s1");
    for (let index = 0; index < 7; index += 1) {
      engine.apply({
        type: "endpoint.detected",
        sessionId: "s1",
        silenceMs: 300,
        detectedAtMs: index * 1000 + 500,
      });
      engine.apply({
        type: "transcript.final",
        sessionId: "s1",
        text: `turn ${index}`,
        startedAtMs: index * 1000,
        endedAtMs: index * 1000 + 400,
        receivedAtMs: index * 1000 + 600,
      });
    }
    expect(engine.context()).toEqual([
      "turn 1",
      "turn 2",
      "turn 3",
      "turn 4",
      "turn 5",
      "turn 6",
    ]);
  });
});
