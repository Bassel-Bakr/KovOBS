import { Component, inject, signal } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { EventsService } from './services/events.service';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet],
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class AppComponent {
  private readonly eventsService = inject(EventsService);

  protected info = signal<string[]>([]);

  constructor() {
    this.eventsService.message().subscribe((message) => this.updateMessage(message));
  }

  private updateMessage(message: string) {
    this.info.update((x) => [...x, message]);
  }
}
