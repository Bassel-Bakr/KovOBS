import { computed, inject, Service, signal } from '@angular/core';
import { EventService } from './event.service';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';

@Service()
export class LogService {
  private readonly eventsService = inject(EventService);
  private readonly internalLogs = signal<string[]>([]);

  readonly logs = computed(() => this.internalLogs());

  constructor() {
    this.eventsService
      .messages()
      .pipe(takeUntilDestroyed())
      .subscribe((message) => {
        this.internalLogs.update((logs) => {
          logs.push(message);
          return logs.slice(logs.length - 1000, logs.length);
        });
      });
  }
}
