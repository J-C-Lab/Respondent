import type { RealtimeEvent } from "./events";

export type ReplyAction =
  | { type: "none" }
  | { type: "start-reply"; transcript: string; context: string[] };

export type ReplyEngineOptions = {
  /**
   * How long an armed endpoint stays valid while waiting for its matching
   * final transcript. If the next final arrives more than this many ms after
   * the endpoint was detected, the endpoint is treated as orphaned (its final
   * was lost) and is discarded instead of triggering a stale reply.
   */
  staleArmMs?: number;
};

const DEFAULT_STALE_ARM_MS = 4000;

export function createReplyEngine(sessionId: string, options: ReplyEngineOptions = {}) {
  const staleArmMs = options.staleArmMs ?? DEFAULT_STALE_ARM_MS;
  let armedAtMs: number | null = null;
  const finalTurns: string[] = [];

  return {
    apply(event: RealtimeEvent): ReplyAction {
      if ("sessionId" in event && event.sessionId !== sessionId) {
        return { type: "none" };
      }

      if (event.type === "endpoint.detected") {
        armedAtMs = event.detectedAtMs;
        return { type: "none" };
      }

      if (event.type === "transcript.final") {
        finalTurns.push(event.text);
        while (finalTurns.length > 6) {
          finalTurns.shift();
        }

        const armedAt = armedAtMs;
        armedAtMs = null;

        if (armedAt !== null && event.receivedAtMs - armedAt <= staleArmMs) {
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
