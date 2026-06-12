import type { RealtimeEvent } from "./events";

export type ReplyAction =
  | { type: "none" }
  | { type: "start-reply"; transcript: string; context: string[] };

export function createReplyEngine(sessionId: string) {
  let endpointArmed = false;
  const finalTurns: string[] = [];

  return {
    apply(event: RealtimeEvent): ReplyAction {
      if ("sessionId" in event && event.sessionId !== sessionId) {
        return { type: "none" };
      }

      if (event.type === "endpoint.detected") {
        endpointArmed = true;
        return { type: "none" };
      }

      if (event.type === "transcript.final") {
        finalTurns.push(event.text);
        while (finalTurns.length > 6) {
          finalTurns.shift();
        }

        if (endpointArmed) {
          endpointArmed = false;
          return {
            type: "start-reply",
            transcript: event.text,
            context: [...finalTurns],
          };
        }
      }

      return { type: "none" };
    },
    context(): string[] {
      return [...finalTurns];
    },
  };
}
