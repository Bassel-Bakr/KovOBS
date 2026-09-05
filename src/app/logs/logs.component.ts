import { Component, computed, effect, ElementRef, inject, signal, viewChild } from '@angular/core';
import { LogService } from '../services/log.service';
import { GlobalService } from '../services/global.service';
import { MatIcon } from '@angular/material/icon';
import { MatIconButton } from '@angular/material/button';
import { MatTooltip } from '@angular/material/tooltip';

type Filter = 'all' | 'clip' | 'error';

const MIN_HEIGHT = 120;

/** Kept clear above the panel so the toolbar and a usable strip of the page
 * below it stay visible however far the handle is dragged. */
const HEADROOM = 180;

const KEYBOARD_STEP = 16;

@Component({
  selector: 'app-logs',
  imports: [MatIcon, MatIconButton, MatTooltip],
  templateUrl: './logs.component.html',
  styleUrl: './logs.component.scss',
  host: { '[style.height.px]': 'globalService.logsHeight()' },
})
export default class LogsComponent {
  private readonly logService = inject(LogService);
  protected readonly globalService = inject(GlobalService);

  private readonly scroller = viewChild<ElementRef<HTMLElement>>('scroller');

  protected readonly filter = signal<Filter>('all');
  protected readonly follow = signal(true);
  protected readonly resizing = signal(false);

  private dragStart: { pointerY: number; height: number } | null = null;

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

  protected startResize(event: PointerEvent): void {
    this.dragStart = { pointerY: event.clientY, height: this.globalService.logsHeight() };
    this.resizing.set(true);
    (event.target as HTMLElement).setPointerCapture(event.pointerId);

    // Suppresses the compatibility mouse events, and with them the text
    // selection that dragging across the page would otherwise start.
    event.preventDefault();
  }

  protected resize(event: PointerEvent): void {
    const start = this.dragStart;

    if (!start) {
      return;
    }

    // The panel is anchored to the bottom, so it grows as the pointer rises.
    this.setHeight(start.height + (start.pointerY - event.clientY));
  }

  protected endResize(event: PointerEvent): void {
    if (!this.dragStart) {
      return;
    }

    this.dragStart = null;
    this.resizing.set(false);
    (event.target as HTMLElement).releasePointerCapture(event.pointerId);
  }

  protected nudge(event: KeyboardEvent): void {
    const step = { ArrowUp: KEYBOARD_STEP, ArrowDown: -KEYBOARD_STEP }[event.key];

    if (step === undefined) {
      return;
    }

    event.preventDefault();
    this.setHeight(this.globalService.logsHeight() + step);
  }

  private setHeight(height: number): void {
    // Recomputed per move so the ceiling follows a window that is being resized.
    const max = Math.max(MIN_HEIGHT, window.innerHeight - HEADROOM);

    this.globalService.logsHeight.set(Math.min(Math.max(height, MIN_HEIGHT), max));
  }
}
