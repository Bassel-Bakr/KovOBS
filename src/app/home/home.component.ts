import { Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { ConfigService } from '../services/config.service';
import { rxResource, takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { form } from '@angular/forms/signals';
import { MatButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatTooltip } from '@angular/material/tooltip';
import { TauriService } from '../services/tauri.service';
import { EventService } from '../services/event.service';
import { combineLatest, of, switchMap, tap } from 'rxjs';
import { GlobalService } from '../services/global.service';
import { PathService } from '../services/path.service';
import { ObsService } from '../services/obs.service';
import { isEqual } from 'lodash-es';
import { Config } from '../models/config';
import { ThemeService } from '../services/theme.service';
import SetupComponent from '../setup/setup.component';
import GameSettingsComponent from './game-settings/game-settings.component';
import AboutSettingsComponent from './about-settings/about-settings.component';
import ObsSettingsComponent from './obs-settings/obs-settings.component';
import ClipSettingsComponent from './clip-settings/clip-settings.component';
import NotificationSettingsComponent from './notification-settings/notification-settings.component';
import AutomationSettingsComponent from './automation-settings/automation-settings.component';
import AdvancedSettingsComponent from './advanced-settings/advanced-settings.component';

type SectionId = 'kovaaks' | 'aimbeast' | 'obs' | 'clips' | 'notifications' | 'automation' | 'advanced' | 'about';

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
    id: 'notifications',
    label: 'Notifications',
    icon: 'notifications',
    title: 'Notifications',
    blurb: 'Desktop notifications for a saved clip, and for anything that goes wrong while you play.',
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
    MatIcon,
    MatTooltip,
    MatButton,
    SetupComponent,
    GameSettingsComponent,
    AboutSettingsComponent,
    ObsSettingsComponent,
    ClipSettingsComponent,
    NotificationSettingsComponent,
    AutomationSettingsComponent,
    AdvancedSettingsComponent,
  ],
  templateUrl: './home.component.html',
  styleUrl: './home.component.scss',
})
export default class HomeComponent {
  private readonly configService = inject(ConfigService);
  private readonly tauriService = inject(TauriService);
  private readonly eventService = inject(EventService);
  private readonly pathService = inject(PathService);
  private readonly obsService = inject(ObsService);
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

  /// Read by the first-run gating as well as the clips section, so it belongs
  /// to the page rather than to either of them.
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

  /**
   * OBS sources for the dropdowns.
   *
   * Filled from two directions: a running session pushes them, and
   * [`loadSources`] asks directly. Without the second, the dropdowns stayed
   * empty until Start -- which is backwards, since picking a source is
   * something you do while setting up.
   */
  protected readonly sources = signal<string[] | null>(null);
  protected readonly loadingSources = signal(false);

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

  protected readonly currentSection = computed(
    () => SECTIONS.find((section) => section.id === this.section()) ?? SECTIONS[0]
  );

  protected readonly processes = computed(() => [
    { id: 'obs' as const, name: 'OBS', running: this.isObsRunning.value() },
    { id: 'kovaaks' as const, name: "KovaaK's", running: this.isKovaaksRunning.value() },
    { id: 'aimbeast' as const, name: 'Aimbeast', running: this.isAimbeastRunning.value() },
  ]);

  constructor() {
    // A running session announces them as it connects.
    this.eventService
      .obsSources()
      .pipe(takeUntilDestroyed())
      .subscribe((sources) => this.sources.set(sources));

    this.loadSources();

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

  /**
   * Asks OBS for its sources directly.
   *
   * A failure is left silent: OBS simply not being open is the ordinary case
   * before Start, and an error banner for it would be noise on every launch.
   */
  protected loadSources(): void {
    this.loadingSources.set(true);

    this.obsService.getSources().subscribe({
      next: (sources) => {
        this.sources.set(sources);
        this.loadingSources.set(false);
      },
      error: () => this.loadingSources.set(false),
    });
  }

  protected isMissing(path: string): boolean {
    return this.missingPaths().has(path);
  }

  protected hasFfmpegArgs(): boolean {
    const { ffmpeg } = this.configForm().value();

    return [ffmpeg.global_args, ffmpeg.input_args, ffmpeg.output_args].some((slot) => slot.trim().length > 0);
  }

  protected selectSection(id: SectionId): void {
    this.section.set(id);
  }

  protected toggleLogs(): void {
    this.globalService.showLogs.update((shown) => !shown);
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
    // Stopping is asynchronous. Capture the whole form now so an intervening
    // config refresh cannot replace the values this click was meant to save.
    const config = structuredClone(this.configForm().value());
    const wasRunning = this.isRunning.value();

    (wasRunning ? this.tauriService.stop() : of(undefined))
      .pipe(
        switchMap(() => this.configService.saveConfig(config)),
        switchMap(() => this.tauriService.setAutoStart(config.auto_start)),
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
