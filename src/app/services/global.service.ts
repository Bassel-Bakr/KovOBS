import { Service, signal } from '@angular/core';

@Service()
export class GlobalService {
  readonly showLogs = signal(false);
}
