import { openUrl } from '@tauri-apps/plugin-opener';
import { Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { ConfigService } from '../services/config.service';
import { rxResource, takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { FieldTree, form, FormField } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';
import { MatFormField, MatHint, MatInput, MatLabel, MatSuffix } from '@angular/material/input';
import { MatButton, MatIconButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatTooltip } from '@angular/material/tooltip';
import { TauriService } from '../services/tauri.service';
import { CacheService } from '../services/cache.service';
import { EventService } from '../services/event.service';
import { combineLatest, of, switchMap, tap } from 'rxjs';
import { GlobalService } from '../services/global.service';
import { FfmpegService } from '../services/ffmpeg.service';
import { PathService } from '../services/path.service';
import { AboutInfo, UpdateInfo, UpdateService } from '../services/update.service';
import { MatProgressSpinner } from '@angular/material/progress-spinner';
import { MatSlideToggle } from '@angular/material/slide-toggle';
import { isEqual } from 'lodash-es';
import { Config, Theme } from '../models/config';
import { ThemeService } from '../services/theme.service';
import SetupComponent from '../setup/setup.component';
import GameSettingsComponent from './game-settings/game-settings.component';

type SectionId = 'kovaaks' | 'aimbeast' | 'obs' | 'clips' | 'automation' | 'advanced' | 'about';

type Section = {
  id: SectionId;
  label: string;
  /** A Material Icons ligature. Unknown names render as their own text, so
   * anything added here has to exist in the self-hosted font. */
  icon: string;
  title: string;
  blurb: string;
};

const SECTIONS: Section[] = [
  {
    id: 'kovaaks',
    label: "KovaaK's",
    icon: 'sports_esports',
    title: "KovaaK's",
    blurb: 'Everything for this game in one place — stats, clips, OBS source and executable.',
  },
  {
    id: 'aimbeast',
    label: 'Aimbeast',
    icon: 'sports_esports',
    title: 'Aimbeast',
    blurb: 'Everything for this game in one place — stats, clips, OBS source and executable.',
  },
  {
    id: 'obs',
    label: 'OBS',
    icon: 'videocam',
    title: 'OBS',
    blurb: 'How KovOBS talks to OBS over the websocket.',
  },
  {
    id: 'clips',
    label: 'Clips',
    icon: 'content_cut',
    title: 'Clips',
    blurb: 'How the replay buffer is trimmed, and what happens to it afterwards.',
  },
  {
    id: 'automation',
    label: 'Automation',
    icon: 'bolt',
    title: 'Automation',
    blurb: 'What KovOBS does on its own while it runs.',
  },
  {
    id: 'advanced',
    label: 'Advanced',
    icon: 'tune',
    title: 'Advanced',
    blurb: 'Cache, scan interval, and things you rarely touch.',
  },
  {
    id: 'about',
    label: 'About',
    icon: 'info',
    title: 'About KovOBS',
    blurb: 'Which version you are running, and whether a newer one exists.',
  },
];

@Component({
  selector: 'app-home',
  imports: [
    FormField,
    MatFormField,
    MatInput,
    MatSuffix,
    MatLabel,
    MatHint,
    MatIconButton,
    MatIcon,
    MatTooltip,
    MatButton,
    MatProgressSpinner,
    MatSlideToggle,
    SetupComponent,
    GameSettingsComponent,
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
  private readonly pathService = inject(PathService);
  private readonly updateService = inject(UpdateService);
  private readonly themeService = inject(ThemeService);
  protected readonly globalService = inject(GlobalService);

  private readonly refresh = signal(new Date());

  /**
   * Tracks if the user hit the stop button themselves to prevent auto start from hijacking the button
   * We only need to set it once and forget, so, no need for signals
   */
  private userClickedStop = false;

  protected readonly sections = SECTIONS;
  protected readonly section = signal<SectionId>('kovaaks');
  protected readonly ffmpegOpen = signal(false);

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

  /** The last value we know is on disk, so edits can be compared against it. */
  private readonly savedConfig = signal<Config | null>(null);

  protected readonly dirty = computed(() => {
    const saved = this.savedConfig();

    return saved != null && !isEqual(saved, this.configForm().value());
  });

  /** Paths in the current form that don't exist, so a field can flag itself. */
  private readonly watchedPaths = computed(() => {
    const value = this.configForm().value();

    return [
      value.stats_folder,
      value.clips_folder,
      value.aimbeast.stats_folder,
      value.aimbeast.clips_folder,
      value.processes.paths.obs,
      value.processes.paths.kovaaks,
      value.processes.paths.aimbeast,
    ];
  });

  private readonly missingPathsResource = rxResource({
    params: () => ({ paths: this.watchedPaths() }),
    stream: ({ params }) => this.pathService.missing(params.paths),
    defaultValue: new Set<string>(),
  });

  /**
   * The checklist is shown on a fresh install only. An existing config predates
   * the `setup_completed` flag and so reads false, which is why the essentials
   * are checked too — someone already set up never sees it.
   */
  protected readonly needsSetup = computed(() => {
    const value = this.config.value();

    if (!value || value.setup_completed) {
      return false;
    }

    const missing = this.missingPaths();
    const hasClips = value.clips_folder.length > 0 && !missing.has(value.clips_folder);
    const hasFfmpeg = this.ffmpegDownloadProgress.value().state === 'Done';
    const hasGame = [value.processes.paths.kovaaks, value.processes.paths.aimbeast].some(
      (path) => path.length > 0 && !missing.has(path)
    );

    return !(hasClips && hasFfmpeg && hasGame);
  });

  protected setupDone(): void {
    this.refresh.set(new Date());
  }

  protected readonly about = rxResource<AboutInfo | null, unknown>({
    stream: () => this.updateService.about(),
    defaultValue: null,
  });

  protected readonly update = signal<UpdateInfo | null>(null);
  protected readonly checking = signal(false);
  protected readonly checkError = signal('');

  protected checkForUpdate(): void {
    this.checking.set(true);
    this.checkError.set('');

    this.updateService.check().subscribe({
      next: (info) => {
        this.update.set(info);
        this.checking.set(false);
      },
      error: (error: unknown) => {
        this.checkError.set(String(error));
        this.checking.set(false);
      },
    });
  }

  /** The releases index, not a specific release. */
  protected openReleases(): void {
    const url = this.about.value()?.releases_url;

    if (url) {
      void openUrl(url);
    }
  }

  /** The page for the release the last check found. */
  protected openLatestRelease(): void {
    const url = this.update()?.release_url;

    if (url) {
      void openUrl(url);
    }
  }

  protected readonly themes: { id: Theme; label: string }[] = [
    { id: 'system', label: 'System' },
    { id: 'light', label: 'Light' },
    { id: 'dark', label: 'Dark' },
  ];

  protected setTheme(theme: Theme): void {
    this.configForm.theme().value.set(theme);
  }

  protected readonly currentSection = computed(
    () => SECTIONS.find((section) => section.id === this.section()) ?? SECTIONS[0]
  );

  protected readonly processes = computed(() => [
    { id: 'obs' as const, name: 'OBS', running: this.isObsRunning.value() },
    { id: 'kovaaks' as const, name: "KovaaK's", running: this.isKovaaksRunning.value() },
    { id: 'aimbeast' as const, name: 'Aimbeast', running: this.isAimbeastRunning.value() },
  ]);

  constructor() {
    effect(() => {
      const value = this.config.value();
      if (value) {
        this.formModel.set(value);
        untracked(() => this.savedConfig.set(structuredClone(value)));
      }
    });

    effect(() => {
      this.themeService.apply(this.configForm.theme().value());
    });

    this.runAutoStartHandler().pipe(takeUntilDestroyed()).subscribe();
  }

  protected readonly missingPaths = computed(() => this.missingPathsResource.value());

  protected isMissing(path: string): boolean {
    return this.missingPaths().has(path);
  }

  protected hasFfmpegArgs(): boolean {
    const { ffmpeg } = this.configForm().value();

    return [ffmpeg.global_args, ffmpeg.input_args, ffmpeg.output_args].some(
      (slot) => slot.trim().length > 0
    );
  }

  protected selectSection(id: SectionId): void {
    this.section.set(id);
  }

  protected toggleFfmpeg(): void {
    this.ffmpegOpen.update((open) => !open);
  }

  protected toggleLogs(): void {
    this.globalService.showLogs.update((shown) => !shown);
  }

  protected clearCache(): void {
    this.cacheService.clearCache().subscribe();
  }

  protected discard(): void {
    const saved = this.savedConfig();

    if (saved) {
      this.formModel.set(structuredClone(saved));
    }
  }

  /**
   * Saving used to require stopping first. Instead, stop and start around the
   * save when the app is running so settings can be changed in place.
   */
  protected save(): void {
    const wasRunning = this.isRunning.value();

    (wasRunning ? this.tauriService.stop() : of(undefined))
      .pipe(
        switchMap(() => this.configService.saveConfig(this.configForm().value())),
        switchMap(() => this.tauriService.setAutoStart(this.configForm.auto_start().value())),
        switchMap(() => (wasRunning ? this.tauriService.start() : of(undefined)))
      )
      .subscribe(() => {
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

  protected toggleRunning(event: MouseEvent): void {
    if (this.isRunning.value()) {
      this.stop(event);
    } else {
      this.start();
    }
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
    this.tauriService.runExe(...params).subscribe({
      // A missing or misconfigured path is reported by the field itself, so a
      // failure here only needs to not become an unhandled rejection.
      error: () => undefined,
    });
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
