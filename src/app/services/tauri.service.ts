import { Service } from '@angular/core';
import { invoke, InvokeArgs } from '@tauri-apps/api/core';
import { defer, Observable, switchMap } from 'rxjs';

import { disable as disableAutoStart, enable as enableAutoStart } from '@tauri-apps/plugin-autostart';

@Service()
export class TauriService {
  call<R>(cmd: string, args?: InvokeArgs): Observable<R> {
    return defer(() => invoke<R>(cmd, args));
  }

  init(): Observable<void> {
    return this.call('init_app');
  }

  start(): Observable<void> {
    return this.call('start_app');
  }

  stop(): Observable<void> {
    return this.call('stop_app');
  }

  restart(): Observable<void> {
    return this.stop().pipe(switchMap(() => this.start()));
  }

  setAutoStart(state: boolean): Observable<void> {
    return defer(() => (state ? enableAutoStart() : disableAutoStart()));
  }

  runExe(exe: 'obs' | 'kovaaks' | 'aimbeast'): Observable<void> {
    return this.call('run_' + exe);
  }
}
