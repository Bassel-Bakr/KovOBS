import { Service } from '@angular/core';
import { invoke, InvokeArgs } from '@tauri-apps/api/core';
import { from, Observable, switchMap } from 'rxjs';

@Service()
export class TauriService {
  call<R>(cmd: string, args?: InvokeArgs): Observable<R> {
    return from(invoke<R>(cmd, args));
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
}
