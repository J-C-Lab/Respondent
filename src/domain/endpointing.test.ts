import { describe, expect, it } from "vitest";
import { chooseEndpointSilenceMs } from "./endpointing";

describe("chooseEndpointSilenceMs", () => {
  it("uses 300 ms for balanced clean speech", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "low",
        recentFalseCuts: 0,
        utteranceMs: 1800,
      }),
    ).toBe(300);
  });

  it("uses 250 ms for very short clean utterances", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "low",
        recentFalseCuts: 0,
        utteranceMs: 650,
      }),
    ).toBe(250);
  });

  it("keeps 900 ms low-noise utterances in the short utterance branch", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "low",
        recentFalseCuts: 0,
        utteranceMs: 900,
      }),
    ).toBe(250);
  });

  it("uses 400 ms for medium noise without false cuts", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "medium",
        recentFalseCuts: 0,
        utteranceMs: 1800,
      }),
    ).toBe(400);
  });

  it("uses 400 ms for low noise with one false cut", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "low",
        recentFalseCuts: 1,
        utteranceMs: 1800,
      }),
    ).toBe(400);
  });

  it("uses 500 ms for high noise", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "high",
        recentFalseCuts: 0,
        utteranceMs: 1800,
      }),
    ).toBe(500);
  });

  it("lets high noise override short utterances", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "high",
        recentFalseCuts: 0,
        utteranceMs: 650,
      }),
    ).toBe(500);
  });

  it("widens to 500 ms after repeated false cuts", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "medium",
        recentFalseCuts: 2,
        utteranceMs: 2400,
      }),
    ).toBe(500);
  });

  it("lets repeated false cuts override short utterances", () => {
    expect(
      chooseEndpointSilenceMs({
        noiseLevel: "low",
        recentFalseCuts: 2,
        utteranceMs: 650,
      }),
    ).toBe(500);
  });
});
