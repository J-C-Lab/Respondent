import type { RealtimeEvent } from "../domain/events";

export type SessionState = {
  sessionId: string;
  status: "idle" | "listening" | "paused" | "ended";
  liveSubtitle: string;
  transcript: string[];
  currentGenerationId: string | null;
  currentSuggestion: string;
  suggestions: string[];
  systemMessages: string[];
};

export function createInitialSessionState(sessionId: string): SessionState {
  return {
    sessionId,
    status: "listening",
    liveSubtitle: "",
    transcript: [],
    currentGenerationId: null,
    currentSuggestion: "",
    suggestions: [],
    systemMessages: [],
  };
}

export function reduceSessionEvent(
  state: SessionState,
  event: RealtimeEvent,
): SessionState {
  if ("sessionId" in event && event.sessionId && event.sessionId !== state.sessionId) {
    return state;
  }

  if (event.type === "transcript.partial") {
    return { ...state, liveSubtitle: event.text };
  }

  if (event.type === "transcript.final") {
    return {
      ...state,
      liveSubtitle: "",
      transcript: [...state.transcript, event.text],
    };
  }

  if (event.type === "reply.started") {
    return {
      ...state,
      currentGenerationId: event.generationId,
      currentSuggestion: "",
    };
  }

  if (
    event.type === "reply.token" &&
    event.generationId === state.currentGenerationId
  ) {
    return {
      ...state,
      currentSuggestion: `${state.currentSuggestion}${event.token}`,
    };
  }

  if (
    event.type === "reply.final" &&
    event.generationId === state.currentGenerationId
  ) {
    return {
      ...state,
      currentSuggestion: event.text,
      suggestions: [...state.suggestions, event.text],
    };
  }

  if (event.type === "system.status") {
    return {
      ...state,
      systemMessages: [...state.systemMessages, event.message],
    };
  }

  return state;
}
