import type { RealtimeEvent } from "../domain/events";

export type RealtimeEmit = (event: RealtimeEvent) => void;
export type StopRealtimeSession = () => void;

type ScheduledEvent = {
  delayMs: number;
  event: RealtimeEvent;
};

export function runMockRealtimeSession(
  sessionId: string,
  emit: RealtimeEmit,
): StopRealtimeSession {
  const generationId = "mock-generation-1";
  const events: ScheduledEvent[] = [
    {
      delayMs: 0,
      event: {
        type: "system.status",
        sessionId,
        level: "info",
        message: "Mock realtime session started",
        receivedAtMs: 0,
      },
    },
    {
      delayMs: 250,
      event: {
        type: "transcript.partial",
        sessionId,
        text: "Could you",
        startedAtMs: 0,
        endedAtMs: 240,
        receivedAtMs: 250,
      },
    },
    {
      delayMs: 650,
      event: {
        type: "transcript.partial",
        sessionId,
        text: "Could you summarize the timeline",
        startedAtMs: 0,
        endedAtMs: 620,
        receivedAtMs: 650,
      },
    },
    {
      delayMs: 950,
      event: {
        type: "endpoint.detected",
        sessionId,
        silenceMs: 300,
        detectedAtMs: 950,
      },
    },
    {
      delayMs: 1050,
      event: {
        type: "transcript.final",
        sessionId,
        text: "Could you summarize the timeline?",
        startedAtMs: 0,
        endedAtMs: 900,
        receivedAtMs: 1050,
      },
    },
    {
      delayMs: 1300,
      event: {
        type: "reply.started",
        sessionId,
        generationId,
        basedOnTranscriptEventId: "mock-transcript-1",
        receivedAtMs: 1300,
      },
    },
    {
      delayMs: 1550,
      event: {
        type: "reply.token",
        sessionId,
        generationId,
        token: "Start with the key dates,",
        receivedAtMs: 1550,
      },
    },
    {
      delayMs: 1800,
      event: {
        type: "reply.token",
        sessionId,
        generationId,
        token: " then call out owners and risks.",
        receivedAtMs: 1800,
      },
    },
    {
      delayMs: 2100,
      event: {
        type: "reply.final",
        sessionId,
        generationId,
        text: "Start with the key dates, then call out owners and risks.",
        receivedAtMs: 2100,
      },
    },
  ];

  const timers = events.map(({ delayMs, event }) =>
    window.setTimeout(() => emit(event), delayMs),
  );

  return () => {
    timers.forEach((timer) => window.clearTimeout(timer));
  };
}
