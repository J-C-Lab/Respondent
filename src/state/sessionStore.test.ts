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

  it("ignores events from another session", () => {
    const state = createInitialSessionState("s1");
    const next = reduceSessionEvent(state, {
      type: "transcript.final",
      sessionId: "s2",
      text: "wrong session",
      startedAtMs: 0,
      endedAtMs: 300,
      receivedAtMs: 340,
    });
    expect(next).toBe(state);
  });

  it("drops reply tokens that do not match the current generation", () => {
    let state = createInitialSessionState("s1");
    state = reduceSessionEvent(state, {
      type: "reply.started",
      sessionId: "s1",
      generationId: "g1",
      basedOnTranscriptEventId: "t1",
      receivedAtMs: 850,
    });
    const next = reduceSessionEvent(state, {
      type: "reply.token",
      sessionId: "s1",
      generationId: "stale-generation",
      token: "ignored",
      receivedAtMs: 900,
    });
    expect(next.currentSuggestion).toBe("");
  });

  it("replaces the streamed suggestion with reply.final text and archives it", () => {
    let state = createInitialSessionState("s1");
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
      token: "Draft th",
      receivedAtMs: 900,
    });
    state = reduceSessionEvent(state, {
      type: "reply.final",
      sessionId: "s1",
      generationId: "g1",
      text: "Draft the corrected reply.",
      receivedAtMs: 960,
    });

    expect(state.currentSuggestion).toBe("Draft the corrected reply.");
    expect(state.suggestions).toEqual(["Draft the corrected reply."]);
  });

  it("ignores reply.final for a non-current generation", () => {
    let state = createInitialSessionState("s1");
    state = reduceSessionEvent(state, {
      type: "reply.started",
      sessionId: "s1",
      generationId: "g1",
      basedOnTranscriptEventId: "t1",
      receivedAtMs: 850,
    });
    const next = reduceSessionEvent(state, {
      type: "reply.final",
      sessionId: "s1",
      generationId: "stale-generation",
      text: "should not land",
      receivedAtMs: 960,
    });
    expect(next.currentSuggestion).toBe("");
    expect(next.suggestions).toEqual([]);
  });

  it("accumulates system messages, including global ones without a session id", () => {
    let state = createInitialSessionState("s1");
    state = reduceSessionEvent(state, {
      type: "system.status",
      sessionId: "s1",
      level: "info",
      message: "session scoped",
      receivedAtMs: 100,
    });
    state = reduceSessionEvent(state, {
      type: "system.status",
      level: "warning",
      message: "global warning",
      receivedAtMs: 200,
    });
    expect(state.systemMessages).toEqual(["session scoped", "global warning"]);
  });

  it("leaves state unchanged for endpoint events", () => {
    const state = createInitialSessionState("s1");
    const next = reduceSessionEvent(state, {
      type: "endpoint.detected",
      sessionId: "s1",
      silenceMs: 300,
      detectedAtMs: 1200,
    });
    expect(next).toBe(state);
  });
});
