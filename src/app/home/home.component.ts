import { Component, inject, signal } from '@angular/core';
import { ConfigService } from '../services/config.service';
import { CacheService } from '../services/cache.service';
import { rxResource } from '@angular/core/rxjs-interop';
import { FieldTree, form, FormField } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';
import { MatFormField, MatInput, MatLabel, MatSuffix } from '@angular/material/input';
import { MatIconButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatCard, MatCardHeader, MatCardTitle } from '@angular/material/card';
import { MatOption, MatSelect } from '@angular/material/select';
import { ObsService } from '../services/obs.service';

@Component({
  selector: 'app-home',
  imports: [
    FormField,
    MatFormField,
    MatInput,
    MatSuffix,
    MatLabel,
    MatIconButton,
    MatIcon,
    MatCardTitle,
    MatCardHeader,
    MatCard,
    MatSelect,
    MatOption,
  ],
  templateUrl: './home.component.html',
  styleUrl: './home.component.scss',
})
export default class HomeComponent {
  private readonly configService = inject(ConfigService);
  private readonly cacheService = inject(CacheService);
  private readonly obsService = inject(ObsService);

  private readonly refresh = signal('');

  protected readonly sources = rxResource({
    stream: () => this.obsService.getSources(),
    defaultValue: [],
  });

  protected readonly config = rxResource({
    params: () => ({ refresh: this.refresh() }),
    stream: () => this.configService.getConfig(),
    defaultValue: this.configService.getEmptyConfig(),
  });

  protected readonly configForm = form(this.config.value);

  protected browse(field: FieldTree<string, string>): void {
    open({
      directory: true,
      multiple: false,
    }).then((path) => field().value.set(path ?? ''));
  }

  protected clearCache(): void {
    this.cacheService.clearCache().subscribe();
  }
}
