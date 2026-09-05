export interface MediaProbe {
  durationSeconds: number | null;
  videoCodec: string | null;
  audioCodec: string | null;
  width: number | null;
  height: number | null;
  hasVideo: boolean;
  hasAudio: boolean;
}
