import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';

@Service()
export class ObsService {
  private readonly tauriService = inject(TauriService);

  getSources(): Observable<string[]> {
    return this.tauriService.callWhenReady('get_obs_sources');
  }
}
