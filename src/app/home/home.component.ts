import { openUrl } from '@tauri-apps/plugin-opener';
import { Component, effect, inject, signal, untracked } from '@angular/core';
import { ConfigService } from '../services/config.service';
import { rxResource } from '@angular/core/rxjs-interop';
import { FieldTree, form, FormField } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';
import { MatFormField, MatInput, MatLabel, MatSuffix } from '@angular/material/input';
import { MatButton, MatIconButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatCard, MatCardHeader, MatCardTitle } from '@angular/material/card';
import { MatOption, MatSelect } from '@angular/material/select';
import { MatCheckbox } from '@angular/material/checkbox';
import { MatTooltip } from '@angular/material/tooltip';
import { MatToolbar } from '@angular/material/toolbar';
import { TauriService } from '../services/tauri.service';
import { CacheService } from '../services/cache.service';
import { EventsService } from '../services/events.service';
import { switchMap } from 'rxjs';

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
    MatCheckbox,
    MatTooltip,
    MatButton,
    MatToolbar,
  ],
  templateUrl: './home.component.html',
  styleUrl: './home.component.scss',
})
export default class HomeComponent {
  private readonly configService = inject(ConfigService);
  private readonly tauriService = inject(TauriService);
  private readonly cacheService = inject(CacheService);
  private readonly eventsService = inject(EventsService);

  private readonly refresh = signal(new Date());

  protected readonly ffmpegForm = form(signal({ args: '' }));

  protected readonly isRunning = rxResource({
    stream: () => this.eventsService.isRunning(),
    defaultValue: false,
  });

  protected readonly isObsRunning = rxResource({
    stream: () => this.eventsService.isObsRunning(),
    defaultValue: false,
  });

  protected readonly isKovaaksRunning = rxResource({
    stream: () => this.eventsService.isKovaaksRunning(),
    defaultValue: false,
  });

  protected readonly sources = rxResource<string[] | null, unknown>({
    stream: () => this.eventsService.obsSources(),
    defaultValue: null,
  });

  protected readonly config = rxResource({
    params: () => ({ refresh: this.refresh() }),
    stream: () => this.configService.getConfig(),
  });

  protected readonly formModel = signal(this.configService.getEmptyConfig());
  protected readonly configForm = form(this.formModel);

  constructor() {
    effect(() => {
      const value = this.config.value();
      if (value) {
        this.formModel.set(value);
      }
    });

    effect(() => {
      const form = this.configForm();
      const args = form.value().ffmpeg_args.join('\n');
      untracked(() => {
        this.ffmpegForm.args().value.set(args);
      });
    });

    effect(() => {
      const form = this.ffmpegForm();
      const args = form.value().args.split('\n');
      untracked(() => {
        this.configForm.ffmpeg_args().value.set(args);
      });
    });
  }

  protected clearCache(): void {
    this.cacheService.clearCache().subscribe();
  }

  protected save(): void {
    this.configService
      .saveConfig(this.configForm().value())
      .pipe(
        switchMap(() => {
          const autoStart = this.configForm.auto_start().value();
          return this.tauriService.setAutoStart(autoStart);
        })
      )
      .subscribe(() => {
        // Handle auto start
        this.refresh.set(new Date());
      });
  }

  protected start(): void {
    this.tauriService.start().subscribe();
  }

  protected stop(): void {
    this.tauriService.stop().subscribe();
  }

  protected browse(field: FieldTree<string, string>): void {
    open({
      directory: true,
      multiple: false,
    }).then((path) => field().value.set(path ?? ''));
  }

  protected browseFile(field: FieldTree<string, string>): void {
    open({ multiple: false }).then((path) => field().value.set(path ?? ''));
  }

  protected openFFmpegHelp(): void {
    void openUrl('https://ffmpeg.org/ffmpeg.html');
  }
}
