import { describe, expect, it } from "vitest";
import { createInitialSessionState, reduceSessionEvent } from "./sessionStore";

describe("session store", () => {
  it("updates subtitle and reply text from realtime events", () => {
    let state = createInitialSessionState("s1");
    state = reduceSessionEvent(state, {
      type: "transcript.partial",
      sessionId: "s1",
      text: "hello",
      startedAtMs: 0,
      endedAtMs: 300,
      receivedAtMs: 340,
    });
    state = reduceSessionEvent(state, {
      type: "transcript.final",
      sessionId: "s1",
      text: "hello there",
      startedAtMs: 0,
      endedAtMs: 600,
      receivedAtMs: 800,
    });
    state = reduceSessionEvent(state, {
      type: "reply.started",
      sessionId: "s1",
      generationId: "g1",
      basedOnTranscriptEventId: "t1",
      receivedAtMs: 850,
    });
    state = reduceSessionEvent(state, {
      type: "reply.token",
      sessionId: "s1",
      generationId: "g1",
      token: "Sure",
      receivedAtMs: 900,
    });
    state = reduceSessionEvent(state, {
      type: "reply.token",
      sessionId: "s1",
      generationId: "g1",
      token: ", I can help.",
      receivedAtMs: 960,
    });

    expect(state.liveSubtitle).toBe("");
    expect(state.transcript).toEqual(["hello there"]);
    expect(state.currentSuggestion).toBe("Sure, I can help.");
  });
});
