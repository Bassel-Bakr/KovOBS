import { Service } from '@angular/core';
import { invoke, InvokeArgs } from '@tauri-apps/api/core';
import { from, Observable } from 'rxjs';

@Service()
export class TauriService {
  call<R>(cmd: string, args?: InvokeArgs): Observable<R> {
    return from(invoke<R>(cmd, args));
  }

  isReady(): Observable<boolean> {
    return this.call('is_ready');
  }
}
