import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  hydrateAppearanceSettings,
  listenAppearanceSettings,
  publishAppearanceSettings,
} from "./appearanceBridge";
import { DEFAULT_APPEARANCE_SETTINGS } from "../state/appearanceSettings";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() =>
  vi.fn(async (_event, handler: (payload: { payload: unknown }) => void) => {
    tauriHandler = handler;
    return () => undefined;
  }),
);

let tauriHandler: ((payload: { payload: unknown }) => void) | null = null;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(),
  listen: listenMock,
}));

class MockBroadcastChannel {
  static channels = new Map<string, Set<MockBroadcastChannel>>();

  private listeners = new Set<(event: MessageEvent<unknown>) => void>();

  constructor(public name: string) {
    const channels =
      MockBroadcastChannel.channels.get(name) ?? new Set<MockBroadcastChannel>();
    channels.add(this);
    MockBroadcastChannel.channels.set(name, channels);
  }

  postMessage(data: unknown) {
    for (const channel of MockBroadcastChannel.channels.get(this.name) ?? []) {
      for (const listener of channel.listeners) {
        listener({ data } as MessageEvent<unknown>);
      }
    }
  }

  addEventListener(_type: "message", listener: (event: MessageEvent) => void) {
    this.listeners.add(listener);
  }

  removeEventListener(
    _type: "message",
    listener: (event: MessageEvent) => void,
  ) {
    this.listeners.delete(listener);
  }
}

describe("appearanceBridge", () => {
  beforeEach(() => {
    vi.stubGlobal("BroadcastChannel", MockBroadcastChannel);
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: { invoke: invokeMock, transformCallback: (cb: () => void) => cb },
      configurable: true,
    });
    invokeMock.mockImplementation(async (command: string, args?: { payload?: unknown }) => {
      if (command === "get_appearance_settings") {
        return DEFAULT_APPEARANCE_SETTINGS;
      }
      if (command === "publish_appearance_settings") {
        return args?.payload;
      }
      return null;
    });
  });

  afterEach(() => {
    localStorage.clear();
    invokeMock.mockReset();
    listenMock.mockReset();
    tauriHandler = null;
    MockBroadcastChannel.channels.clear();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.unstubAllGlobals();
  });

  it("broadcasts theme changes to other listeners immediately in the browser", async () => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;

    const onChange = vi.fn();
    const unlisten = await listenAppearanceSettings(onChange);

    await publishAppearanceSettings({
      windowOpacity: 72,
      windowBlur: 24,
      appearanceTheme: "light",
    });

    expect(onChange).toHaveBeenCalledWith({
      windowOpacity: 72,
      windowBlur: 24,
      appearanceTheme: "light",
    });

    unlisten();
  });

  it("publishes through the native backend in Tauri", async () => {
    await publishAppearanceSettings({
      windowOpacity: 80,
      windowBlur: 16,
      appearanceTheme: "light",
    });

    expect(invokeMock).toHaveBeenCalledWith("publish_appearance_settings", {
      payload: {
        windowOpacity: 80,
        windowBlur: 16,
        appearanceTheme: "light",
      },
    });
  });

  it("seeds native settings from local storage on the main window", async () => {
    localStorage.setItem(
      "respondent.appearance",
      JSON.stringify({
        windowOpacity: 86,
        windowBlur: 18,
        appearanceTheme: "light",
      }),
    );

    const settings = await hydrateAppearanceSettings(
      {
        windowOpacity: 86,
        windowBlur: 18,
        appearanceTheme: "light",
      },
      false,
    );

    expect(settings.appearanceTheme).toBe("light");
    expect(invokeMock).toHaveBeenCalledWith("publish_appearance_settings", {
      payload: {
        windowOpacity: 86,
        windowBlur: 18,
        appearanceTheme: "light",
      },
    });
  });

  it("loads native settings in dialog windows without seeding", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_appearance_settings") {
        return {
          windowOpacity: 72,
          windowBlur: 24,
          appearanceTheme: "light",
        };
      }
      return null;
    });

    const settings = await hydrateAppearanceSettings(
      DEFAULT_APPEARANCE_SETTINGS,
      true,
    );

    expect(settings.appearanceTheme).toBe("light");
    expect(invokeMock).not.toHaveBeenCalledWith(
      "publish_appearance_settings",
      expect.anything(),
    );
  });

  it("forwards tauri appearance events to listeners", async () => {
    const onChange = vi.fn();
    const unlisten = await listenAppearanceSettings(onChange);

    tauriHandler?.({
      payload: {
        windowOpacity: 80,
        windowBlur: 20,
        appearanceTheme: "dark",
      },
    });

    expect(onChange).toHaveBeenCalledWith({
      windowOpacity: 80,
      windowBlur: 20,
      appearanceTheme: "dark",
    });

    unlisten();
  });
});
