import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';

@Service()
export class NotificationService {
  private readonly tauriService = inject(TauriService);

  sendTest(): Observable<void> {
    return this.tauriService.call<void>('test_notification');
  }
}
