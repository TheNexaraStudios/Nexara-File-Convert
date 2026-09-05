export interface EngineReadiness {
  id: string;
  displayName: string;
  version: string;
  license: string;
  ready: boolean;
}

export type ProvisionPhase = "pending" | "downloading" | "verifying" | "extracting" | "installing" | "ready" | "failed";

export interface ProvisionEvent {
  id: string;
  displayName: string;
  phase: ProvisionPhase;
  bytesDownloaded: number | null;
  bytesTotal: number | null;
  message: string | null;
}

export interface ProvisionFailure {
  id: string;
  message: string;
}
