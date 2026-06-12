export type LatencyMetric =
  | "asr_partial"
  | "asr_final"
  | "endpoint"
  | "reply_ttft"
  | "reply_complete";
export type LatencyClass = "target" | "slow";

const thresholds: Record<LatencyMetric, number> = {
  asr_partial: 800,
  asr_final: 1800,
  endpoint: 500,
  reply_ttft: 1500,
  reply_complete: 3000,
};

export function classifyLatency(metric: LatencyMetric, valueMs: number): LatencyClass {
  return valueMs <= thresholds[metric] ? "target" : "slow";
}
