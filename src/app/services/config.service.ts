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
      auto_start: false,
      stats_folder: '',
      clips_folder: '',
      obs: {
        host: 'localhost',
        port: 4455,
        password: '',
        source_name: "KovaaK's",
      },
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
