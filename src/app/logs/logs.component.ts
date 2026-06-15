import { Component, effect, ElementRef, inject, signal } from '@angular/core';
import { EventsService } from '../services/events.service';

@Component({
  selector: 'app-logs',
  imports: [],
  templateUrl: './logs.component.html',
  styleUrl: './logs.component.scss',
})
export default class LogsComponent {
  private readonly eventsService = inject(EventsService);

  protected readonly logs = signal<string[]>([]);
  private readonly host = inject(ElementRef<HTMLElement>);

  private updateMessage(message: string) {
    this.logs.update((x) => [...x, message]);
  }

  constructor() {
    this.eventsService.message().subscribe((message) => this.updateMessage(message));

    // Scroll to the bottom on update
    effect(() => {
      if (this.logs().length > 0) {
        const div = this.host.nativeElement;
        requestAnimationFrame(() => div.scrollTo({ top: div.scrollHeight, behavior: 'smooth' }));
      }
    });
  }
}
