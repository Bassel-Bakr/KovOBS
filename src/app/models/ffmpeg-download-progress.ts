export type FFmpegDownloadProgress = {
  state: FFmpegDownloadProgressState;
  progress: number;
};

type FFmpegDownloadProgressState = 'Unknown' | 'Starting' | 'Downloading' | 'Unpacking' | 'Done' | 'NotDone';
