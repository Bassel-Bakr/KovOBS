import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';

export type UpdateInfo = {
  current: string;
  latest: string;
  release_url: string;
  update_available: boolean;
};

@Service()
export class UpdateService {
  private readonly tauriService = inject(TauriService);

  version(): Observable<string> {
    return this.tauriService.call<string>('app_version');
  }

  /** Only called when the user asks, so the app never phones home on its own. */
  check(): Observable<UpdateInfo> {
    return this.tauriService.call<UpdateInfo>('check_for_update');
  }
}
