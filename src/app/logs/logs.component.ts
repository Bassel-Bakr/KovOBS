import { Component, effect, ElementRef, inject } from '@angular/core';
import { EventsService } from '../services/events.service';
import { LogService } from '../services/log.service';

@Component({
  selector: 'app-logs',
  imports: [],
  templateUrl: './logs.component.html',
  styleUrl: './logs.component.scss',
})
export default class LogsComponent {
  private readonly eventsService = inject(EventsService);
  private readonly logService = inject(LogService);

  private readonly host = inject(ElementRef<HTMLElement>);

  protected readonly logs = this.logService.logs;

  constructor() {
    // Scroll to the bottom on update
    effect(() => {
      if (this.logs().length > 0) {
        const div = this.host.nativeElement;
        requestAnimationFrame(() => div.scrollTo({ top: div.scrollHeight, behavior: 'smooth' }));
      }
    });
  }
}
