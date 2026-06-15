import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';

@Service()
export class CacheService {
  private readonly tauriService = inject(TauriService);

  clearCache(): Observable<void> {
    return this.tauriService.callWhenReady<void>('clear_cache');
  }
}
