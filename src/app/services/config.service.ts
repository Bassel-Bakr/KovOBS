import { inject, Service } from '@angular/core';
import { map, Observable } from 'rxjs';
import { TauriService } from './tauri.service';
import { Config, DEFAULT_CONFIG } from '../models/config';
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

  /**
   * The shape the form starts from, before the real config arrives.
   *
   * Generated from `AppConfig::default()`, so it cannot drift from the
   * backend. Cloned because `merge` writes into its first argument.
   */
  getEmptyConfig(): Config {
    return structuredClone(DEFAULT_CONFIG);
  }
}
