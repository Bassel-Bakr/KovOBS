export type Config = {
  stats_folder: string;
  clips_folder: string;
  obs_host: string;
  obs_port: number;
  obs_password: string;
  obs_replay_folder: string;
  obs_source_name: string;
  trim_padding_start: number;
  trim_padding_end: number;
  delete_after_trimming: boolean;
  only_pb: boolean;
  cache_version: string;
  cache_file: string;
  screenshot: {
    enabled: boolean;
  };
  ffmpeg_args: string[];
};
