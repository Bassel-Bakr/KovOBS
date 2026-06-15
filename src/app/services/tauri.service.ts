import { Service } from '@angular/core';
import { invoke, InvokeArgs } from '@tauri-apps/api/core';
import { Observable, ReplaySubject, switchMap, take } from 'rxjs';

@Service()
export class TauriService {
  readonly isReady = new ReplaySubject<boolean>(1);

  call<R>(cmd: string, args?: InvokeArgs): Observable<R> {
    return this.isReady.pipe(
      take(1),
      switchMap(() => invoke<R>(cmd, args))
    );
  }
}
