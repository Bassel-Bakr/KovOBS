import { Component, computed, effect, inject, output, signal, untracked } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { open } from '@tauri-apps/plugin-dialog';
import { MatFormField, MatHint, MatInput, MatLabel } from '@angular/material/input';
import { MatButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatProgressSpinner } from '@angular/material/progress-spinner';
import { ConfigService } from '../services/config.service';
import { EventService } from '../services/event.service';
import { FfmpegService } from '../services/ffmpeg.service';
import { PathService } from '../services/path.service';
import { Config } from '../models/config';

type StepId = 'clips' | 'obs' | 'ffmpeg' | 'game';

@Component({
  selector: 'app-setup',
  imports: [MatFormField, MatLabel, MatHint, MatInput, MatButton, MatIcon, MatProgressSpinner],
  templateUrl: './setup.component.html',
  styleUrl: './setup.component.scss',
})
export default class SetupComponent {
  private readonly configService = inject(ConfigService);
  private readonly eventService = inject(EventService);
  private readonly ffmpegService = inject(FfmpegService);
  private readonly pathService = inject(PathService);

  readonly done = output<void>();

  private readonly refresh = signal(new Date());

  private readonly loaded = rxResource({
    params: () => ({ refresh: this.refresh() }),
    stream: () => this.configService.getConfig(),
  });

  /** Working copy, so a picked folder shows immediately. */
  protected readonly config = signal<Config | null>(null);

  protected readonly ffmpegProgress = rxResource({
    stream: () => this.eventService.ffmpegDownloadProgress(),
    defaultValue: { state: 'NotDone', progress: 0 },
  });

  protected readonly sources = rxResource<string[] | null, unknown>({
    stream: () => this.eventService.obsSources(),
    defaultValue: null,
  });

  private readonly missingPaths = rxResource({
    params: () => {
      const config = this.config();

      return {
        paths: config ? [config.clips_folder, config.processes.paths.kovaaks, config.processes.paths.aimbeast] : [],
      };
    },
    stream: ({ params }) => this.pathService.missing(params.paths),
    defaultValue: new Set<string>(),
  });

  protected readonly clipsDone = computed(() => {
    const folder = this.config()?.clips_folder ?? '';

    return folder.length > 0 && !this.missingPaths.value().has(folder);
  });

  protected readonly obsDone = computed(() => this.sources.value() != null);

  protected readonly ffmpegDone = computed(() => this.ffmpegProgress.value().state === 'Done');

  protected readonly ffmpegBusy = computed(() => {
    const state = this.ffmpegProgress.value().state;

    return state !== 'Done' && state !== 'NotDone' && state !== 'Unknown';
  });

  protected readonly gameDone = computed(() => {
    const paths = this.config()?.processes.paths;

    if (!paths) {
      return false;
    }

    const missing = this.missingPaths.value();
    const found = [paths.kovaaks, paths.aimbeast].filter((path) => path.length > 0 && !missing.has(path));

    return found.length > 0;
  });

  protected readonly steps = computed(() => [
    { id: 'clips' as StepId, done: this.clipsDone() },
    { id: 'obs' as StepId, done: this.obsDone() },
    { id: 'ffmpeg' as StepId, done: this.ffmpegDone() },
    { id: 'game' as StepId, done: this.gameDone() },
  ]);

  protected readonly completed = computed(() => this.steps().filter((step) => step.done).length);
  protected readonly total = computed(() => this.steps().length);
  protected readonly ready = computed(() => this.completed() === this.total());
  protected readonly progress = computed(() => Math.round((this.completed() / this.total()) * 100));

  constructor() {
    effect(() => {
      const value = this.loaded.value();

      if (value) {
        untracked(() => this.config.set(structuredClone(value)));
      }
    });
  }

  protected async pickClipsFolder(): Promise<void> {
    const folder = await open({ directory: true, multiple: false });

    if (folder != null) {
      this.patch((config) => ({
        ...config,
        clips_folder: folder,
        aimbeast: { ...config.aimbeast, clips_folder: folder },
      }));
    }
  }

  protected async pickGame(which: 'kovaaks' | 'aimbeast'): Promise<void> {
    const file = await open({ multiple: false });

    if (file != null) {
      this.patch((config) => ({
        ...config,
        processes: { ...config.processes, paths: { ...config.processes.paths, [which]: file } },
      }));
    }
  }

  protected setPassword(value: string): void {
    this.patch((config) => ({ ...config, obs: { ...config.obs, password: value } }));
  }

  protected downloadFFmpeg(): void {
    this.ffmpegService.download().subscribe();
  }

  protected finish(): void {
    const config = this.config();

    if (!config) {
      return;
    }

    this.configService.saveConfig({ ...config, setup_completed: true }).subscribe(() => this.done.emit());
  }

  /** Applies an edit and persists it, so nothing is lost if setup is abandoned. */
  private patch(update: (config: Config) => Config): void {
    const current = this.config();

    if (!current) {
      return;
    }

    const next = update(current);
    this.config.set(next);
    this.configService.saveConfig(next).subscribe();
  }
}
