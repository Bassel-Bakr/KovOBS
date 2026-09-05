import { openUrl } from '@tauri-apps/plugin-opener';
import { Component, effect, inject, signal, untracked } from '@angular/core';
import { ConfigService } from '../services/config.service';
import { rxResource, takeUntilDestroyed } from '@angular/core/rxjs-interop';
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
import { EventService } from '../services/event.service';
import { combineLatest, switchMap, tap } from 'rxjs';
import { MatChip } from '@angular/material/chips';
import { GlobalService } from '../services/global.service';
import { FfmpegService } from '../services/ffmpeg.service';
import { MatProgressSpinner } from '@angular/material/progress-spinner';

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
    MatChip,
    MatProgressSpinner,
  ],
  templateUrl: './home.component.html',
  styleUrl: './home.component.scss',
})
export default class HomeComponent {
  private readonly configService = inject(ConfigService);
  private readonly tauriService = inject(TauriService);
  private readonly cacheService = inject(CacheService);
  private readonly eventService = inject(EventService);
  private readonly ffmpegService = inject(FfmpegService);
  protected readonly globalService = inject(GlobalService);

  private readonly refresh = signal(new Date());

  /**
   * Tracks if the user hit the stop button themselves to prevent auto start from hijacking the button
   * We only need to set it once and forget, so, no need for signals
   */
  private userClickedStop = false;

  protected readonly ffmpegForm = form(signal({ global: '', input: '', output: '' }));

  protected readonly ffmpegDownloadProgress = rxResource({
    stream: () => this.eventService.ffmpegDownloadProgress(),
    defaultValue: { state: 'NotDone', progress: 0 },
  });

  protected readonly isRunning = rxResource({
    stream: () => this.eventService.isRunning(),
    defaultValue: false,
  });

  protected readonly isObsRunning = rxResource({
    stream: () => this.eventService.isObsRunning(),
    defaultValue: false,
  });

  protected readonly isKovaaksRunning = rxResource({
    stream: () => this.eventService.isKovaaksRunning(),
    defaultValue: false,
  });

  protected readonly isAimbeastRunning = rxResource({
    stream: () => this.eventService.isAimbeastRunning(),
    defaultValue: false,
  });

  protected readonly sources = rxResource<string[] | null, unknown>({
    stream: () => this.eventService.obsSources(),
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
      const { ffmpeg } = this.configForm().value();
      untracked(() => {
        this.ffmpegForm.global().value.set(ffmpeg.global_args.join('\n'));
        this.ffmpegForm.input().value.set(ffmpeg.input_args.join('\n'));
        this.ffmpegForm.output().value.set(ffmpeg.output_args.join('\n'));
      });
    });

    effect(() => {
      const { global, input, output } = this.ffmpegForm().value();
      untracked(() => {
        this.configForm.ffmpeg.global_args().value.set(toArgs(global));
        this.configForm.ffmpeg.input_args().value.set(toArgs(input));
        this.configForm.ffmpeg.output_args().value.set(toArgs(output));
      });
    });

    this.runAutoStartHandler().pipe(takeUntilDestroyed()).subscribe();
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

  protected stop(event: MouseEvent): void {
    if (event.isTrusted) {
      this.userClickedStop = true;
    }
    this.tauriService.stop().subscribe();
  }

  protected browse(field: FieldTree<string, string>): void {
    open({
      directory: true,
      multiple: false,
    }).then((path) => {
      if (path != null) {
        field().value.set(path ?? '');
      }
    });
  }

  protected browseFile(field: FieldTree<string, string>): void {
    open({ multiple: false }).then((path) => {
      if (path != null) {
        field().value.set(path ?? '');
      }
    });
  }

  protected openFFmpegHelp(): void {
    void openUrl('https://ffmpeg.org/ffmpeg.html');
  }

  protected downloadFFmpeg(): void {
    this.ffmpegService.download().subscribe();
  }

  protected deleteFFmpeg(): void {
    this.ffmpegService.remove().subscribe();
  }

  protected runExe(...params: Parameters<typeof this.tauriService.runExe>): void {
    this.tauriService.runExe(...params).subscribe();
  }

  protected runAutoStartHandler() {
    return combineLatest([
      this.eventService.isAimbeastRunning(),
      this.eventService.isKovaaksRunning(),
      this.eventService.isObsRunning(),
      this.eventService.isRunning(),
      this.eventService.config(),
    ]).pipe(
      tap(([isAimbeastRunning, isKovaaksRunning, isObsRunning, isRunning, config]) => {
        // Don't proceed unless auto start is enabled.
        if (!config.auto_start) {
          return;
        }

        // If KovaaK's or Aimbeast aren't running, there is nothing to do.
        if (!isKovaaksRunning && !isAimbeastRunning) {
          return;
        }

        // If OBS is not running, open it.
        if (!isObsRunning) {
          return this.runExe('obs');
        }

        // If we're not running, what are we waiting for?!
        if (!isRunning && !this.userClickedStop) {
          return this.start();
        }
      })
    );
  }
}

function toArgs(value: string): string[] {
  return value
    .split('\n')
    .map((arg) => arg.trim())
    .filter((arg) => arg.length > 0);
}
