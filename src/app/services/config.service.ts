import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';
import { Config } from '../models/config';

@Service()
export class ConfigService {
  private readonly tauriService = inject(TauriService);

  getConfig(): Observable<Config> {
    return this.tauriService.call<Config>('get_config');
  }

  saveConfig(config: Config): Observable<Config> {
    return this.tauriService.call<Config>('save_config', { config });
  }

  getEmptyConfig(): Config {
    return {
      stats_folder: '',
      clips_folder: '',
      obs_host: 'localhost',
      obs_port: 4455,
      obs_password: '',
      obs_source_name: "KovaaK's",
      trim_padding_start: 0,
      trim_padding_end: 5,
      delete_after_trimming: false,
      only_pb: false,
      cache_version: '1.0.0',
      cache_file: 'cache.json',
      screenshot: {
        enabled: false,
      },
      ffmpeg_args: ['-c', 'copy'],
    };
  }
}
