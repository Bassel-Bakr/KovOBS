export type Config = {
  auto_start: boolean;
  stats_folder: string;
  clips_folder: string;
  obs: {
    host: string;
    port: number;
    password: string;
    source_name: string;
  };
  aimbeast: {
    stats_folder: string;
    clips_folder: string;
    obs_source_name: string;
  };
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
  processes: {
    scan_interval_secs: number;
    paths: {
      obs: string;
      kovaaks: string;
      aimbeast: string;
    };
  };
};
