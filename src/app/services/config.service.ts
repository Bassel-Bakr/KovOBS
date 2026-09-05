import { inject, Service } from '@angular/core';
import { map, Observable } from 'rxjs';
import { TauriService } from './tauri.service';
import { Config } from '../models/config';
import { merge } from 'lodash-es';

@Service()
export class ConfigService {
  private readonly tauriService = inject(TauriService);

  getConfig(): Observable<Config> {
    return this.tauriService.call<Config>('get_config').pipe(map((config) => merge(this.getEmptyConfig(), config)));
  }

  saveConfig(config: Config): Observable<Config> {
    return this.tauriService.call<Config>('save_config', { config });
  }

  getEmptyConfig(): Config {
    return {
      auto_start: false,
      setup_completed: true,
      theme: 'system',
      stats_folder: '',
      clips_folder: '',
      obs: {
        host: 'localhost',
        port: 4455,
        password: '',
        source_name: "KovaaK's",
      },
      aimbeast: {
        clips_folder: '',
        stats_folder: '',
        obs_source_name: 'Aimbeast',
      },
      trim: true,
      trim_padding_start: 0,
      trim_padding_end: 5,
      delete_after_trimming: false,
      only_pb: false,
      cache_version: '1.0.0',
      cache_file: 'cache.json',
      screenshot: {
        enabled: false,
      },
      ffmpeg: {
        global_args: '',
        input_args: '',
        output_args: '',
      },
      processes: {
        scan_interval_secs: 1,
        paths: {
          obs: 'obs64.exe',
          kovaaks: 'FPSAimTrainer-Win64-Shipping.exe',
          aimbeast: 'Aimbeast-Win64-Shipping.exe',
        },
      },
    };
  }
}
