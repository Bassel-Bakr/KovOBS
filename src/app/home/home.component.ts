import { Component, inject, signal } from '@angular/core';
import { ConfigService } from '../services/config.service';
import { CacheService } from '../services/cache.service';
import { rxResource } from '@angular/core/rxjs-interop';
import { JsonPipe } from '@angular/common';
import { form, FormField } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';

@Component({
  selector: 'app-home',
  imports: [JsonPipe, FormField],
  templateUrl: './home.component.html',
  styleUrl: './home.component.scss',
})
export default class HomeComponent {
  private readonly configService = inject(ConfigService);
  private readonly cacheService = inject(CacheService);

  private readonly refresh = signal('');

  protected readonly config = rxResource({
    params: () => ({ refresh: this.refresh() }),
    stream: () => this.configService.getConfig(),
    defaultValue: this.configService.getEmptyConfig(),
  });

  protected readonly configForm = form(this.config.value);

  protected browseStats(): void {
    open({
      directory: true,
      multiple: false,
    }).then((path) => this.configForm.stats_folder().value.set(path ?? ''));
  }

  protected clearCache(): void {
    this.cacheService.clearCache().subscribe();
  }

  protected refreshConfig(): void {
    this.refresh.set(new Date().toISOString());
  }
}
