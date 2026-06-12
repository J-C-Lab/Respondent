import { invoke } from "@tauri-apps/api/core";

export type OutputDevice = {
  id: string;
  name: string;
  is_default: boolean;
};

export async function listAudioOutputDevices(): Promise<OutputDevice[]> {
  return invoke<OutputDevice[]>("list_audio_output_devices");
}

export async function startNativeSession(
  title: string,
  outputDeviceId: string,
): Promise<string> {
  return invoke<string>("start_session", { title, outputDeviceId });
}

export async function endNativeSession(sessionId: string): Promise<void> {
  await invoke("end_session", { sessionId });
}
