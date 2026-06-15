import { Service } from '@angular/core';
import { invoke, InvokeArgs } from '@tauri-apps/api/core';
import { EMPTY, expand, from, Observable, switchMap } from 'rxjs';

@Service()
export class TauriService {
  start(): Observable<void> {
    return this.call('start_app');
  }

  stop(): Observable<void> {
    return this.call('stop_app');
  }

  restart(): Observable<void> {
    return this.stop().pipe(switchMap(() => this.start()));
  }

  call<R>(cmd: string, args?: InvokeArgs): Observable<R> {
    return this.waitUntilReady().pipe(switchMap(() => invoke<R>(cmd, args)));
  }

  private waitUntilReady(): Observable<boolean> {
    return this.isReady().pipe(expand((isReady) => (isReady ? EMPTY : this.isReady())));
  }

  private isReady(): Observable<boolean> {
    return from(invoke<boolean>('is_ready'));
  }
}
