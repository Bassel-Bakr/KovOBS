import { Component, inject, signal } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { EventsService } from './services/events.service';
import { ConfigService } from './services/config.service';
import { rxResource } from '@angular/core/rxjs-interop';
import { CacheService } from './services/cache.service';
import { JsonPipe } from '@angular/common';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, JsonPipe],
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class AppComponent {
  private readonly eventsService = inject(EventsService);
  private readonly configService = inject(ConfigService);
  private readonly cacheService = inject(CacheService);

  private readonly refresh = signal('');

  protected readonly config = rxResource({
    params: () => ({ refresh: this.refresh() }),
    stream: () => this.configService.getConfig(),
  });

  protected info = signal<string[]>([]);

  constructor() {
    this.eventsService.message().subscribe((message) => this.updateMessage(message));
  }

  protected clearCache(): void {
    this.cacheService.clearCache().subscribe();
  }

  protected refreshConfig(): void {
    this.refresh.set(new Date().toISOString());
  }

  private updateMessage(message: string) {
    this.info.update((x) => [...x, message]);
  }
}
