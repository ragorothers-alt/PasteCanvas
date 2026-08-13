import { invoke } from "@tauri-apps/api/core";

export interface SessionInfo {
  id: string;
  name: string | null;
  state: string; // "active" | "paused" | "finished"
  startedAt: number;
  itemCount: number;
}

export async function startSession(name?: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("start_capture_session", { request: { name } });
}

export async function pauseSession(): Promise<SessionInfo> {
  return invoke<SessionInfo>("pause_capture_session");
}

export async function resumeSession(): Promise<SessionInfo> {
  return invoke<SessionInfo>("resume_capture_session");
}

export async function finishSession(): Promise<SessionInfo> {
  return invoke<SessionInfo>("finish_capture_session");
}

export async function getSession(): Promise<SessionInfo | null> {
  return invoke<SessionInfo | null>("get_capture_session");
}
