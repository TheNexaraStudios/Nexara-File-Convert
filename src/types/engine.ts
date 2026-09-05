export type EngineAvailability = "available" | "unavailable";

export interface EngineInfo {
  id: string;
  name: string;
  binary: string;
  availability: EngineAvailability;
  implemented: boolean;
  description: string;
}
