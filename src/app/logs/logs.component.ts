import { Component, computed, effect, ElementRef, inject, signal, viewChild } from '@angular/core';
import { LogService } from '../services/log.service';
import { GlobalService } from '../services/global.service';
import { MatIcon } from '@angular/material/icon';
import { MatIconButton } from '@angular/material/button';
import { MatTooltip } from '@angular/material/tooltip';
import { LogLevel } from '../models/log-entry';

type Filter = 'all' | 'clip' | 'error';

@Component({
  selector: 'app-logs',
  imports: [MatIcon, MatIconButton, MatTooltip],
  templateUrl: './logs.component.html',
  styleUrl: './logs.component.scss',
})
export default class LogsComponent {
  private readonly logService = inject(LogService);
  protected readonly globalService = inject(GlobalService);

  private readonly scroller = viewChild<ElementRef<HTMLElement>>('scroller');

  protected readonly filter = signal<Filter>('all');
  protected readonly follow = signal(true);

  protected readonly filters: { id: Filter; label: string }[] = [
    { id: 'all', label: 'All' },
    { id: 'clip', label: 'Clips' },
    { id: 'error', label: 'Errors' },
  ];

  protected readonly entries = computed(() => {
    const filter = this.filter();
    const logs = this.logService.logs();

    return filter === 'all' ? logs : logs.filter((entry) => entry.level === filter);
  });

  constructor() {
    effect(() => {
      // Depend on the rendered entries so a filter change re-pins too.
      const count = this.entries().length;

      if (!this.follow() || count === 0) {
        return;
      }

      const element = this.scroller()?.nativeElement;

      if (element) {
        requestAnimationFrame(() => element.scrollTo({ top: element.scrollHeight }));
      }
    });
  }

  protected setFilter(filter: Filter): void {
    this.filter.set(filter);
  }

  protected toggleFollow(): void {
    this.follow.update((follow) => !follow);
  }

  protected clear(): void {
    this.logService.clear();
  }

  protected copy(): void {
    const text = this.entries()
      .map((entry) => `${entry.time} ${entry.text}`)
      .join('\n');

    void navigator.clipboard.writeText(text);
  }

  protected close(): void {
    this.globalService.showLogs.set(false);
  }

  protected trackLevel(level: LogLevel): string {
    return level;
  }
}
