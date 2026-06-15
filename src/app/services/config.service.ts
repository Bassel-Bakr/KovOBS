import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';
import { Config } from '../models/config';

@Service()
export class ConfigService {
  private readonly tauriService = inject(TauriService);

  getConfig(): Observable<Config> {
    return this.tauriService.callWhenReady<Config>('get_config');
  }

  getEmptyConfig(): Config {
    return {
      stats_folder: '',
      clips_folder: '',
      obs_host: 'localhost',
      obs_port: 4455,
      obs_password: '',
      obs_replay_folder: '',
      obs_source_name: '',
      trim_padding_start: 0,
      trim_padding_end: 0,
      delete_after_trimming: false,
      only_pb: false,
      cache_version: '',
      cache_file: '',
      screenshot: {
        enabled: false,
      },
      ffmpeg_args: [],
    };
  }
}
