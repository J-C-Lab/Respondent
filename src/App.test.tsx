import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

describe("App", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("streams a suggested reply after starting a mock session", async () => {
    render(<App />);

    fireEvent.click(screen.getByTitle("Start"));

    // Partial subtitle shows while the speaker is mid-sentence.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(700);
    });
    expect(
      screen.getByText("Could you summarize the timeline"),
    ).toBeInTheDocument();

    // After endpoint + final + reply tokens, the streamed suggestion lands.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1600);
    });
    expect(
      screen.getByText(
        "Start with the key dates, then call out owners and risks.",
      ),
    ).toBeInTheDocument();
  });

  it("marks the session saved after End", async () => {
    render(<App />);

    fireEvent.click(screen.getByTitle("Start"));
    fireEvent.click(screen.getByTitle("End"));

    expect(screen.getByText("Saved")).toBeInTheDocument();
  });
});
