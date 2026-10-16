import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';

/**
 * A program running now. For a Windows game that Proton runs on Linux, `path`
 * is the game's own executable, not Wine's.
 */
export type RunningProgram = {
  name: string;
  path: string;
};

@Service()
export class ProcessService {
  private readonly tauriService = inject(TauriService);

  /** Every program running now, once each, by name. */
  running(): Observable<RunningProgram[]> {
    return this.tauriService.call<RunningProgram[]>('list_running_programs');
  }
}
