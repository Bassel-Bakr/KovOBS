import { ComponentFixture, TestBed } from '@angular/core/testing';
import { signal } from '@angular/core';
import { of, Subject } from 'rxjs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import HomeComponent from './home.component';
import { ConfigService } from '../services/config.service';
import { TauriService } from '../services/tauri.service';
import { EventService } from '../services/event.service';
import { PathService } from '../services/path.service';
import { ObsService } from '../services/obs.service';
import { ThemeService } from '../services/theme.service';
import { GlobalService } from '../services/global.service';
import { Config, DEFAULT_CONFIG } from '../models/config';

describe('HomeComponent', () => {
  let fixture: ComponentFixture<HomeComponent>;
  let stop: Subject<void>;
  let saveConfig: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    const config = structuredClone(DEFAULT_CONFIG);
    config.setup_completed = true;
    stop = new Subject<void>();
    saveConfig = vi.fn(() => of(config));

    await TestBed.configureTestingModule({
      imports: [HomeComponent],
      providers: [
        {
          provide: ConfigService,
          useValue: { getConfig: () => of(config), getEmptyConfig: () => structuredClone(config), saveConfig },
        },
        {
          provide: TauriService,
          useValue: {
            stop: () => stop,
            start: () => of(undefined),
            setAutoStart: () => of(undefined),
            runExe: () => of(undefined),
          },
        },
        {
          provide: EventService,
          useValue: {
            ffmpegDownloadProgress: () => of({ state: 'NotDone', progress: 0 }),
            isRunning: () => of(true),
            isObsRunning: () => of(false),
            isKovaaksRunning: () => of(false),
            isAimbeastRunning: () => of(false),
            obsSources: () => of([]),
            config: () => of(config),
          },
        },
        { provide: PathService, useValue: { missing: () => of(new Set<string>()) } },
        { provide: ObsService, useValue: { getSources: () => of([]) } },
        { provide: ThemeService, useValue: { apply: () => undefined } },
        { provide: GlobalService, useValue: { showLogs: signal(false) } },
      ],
    }).compileComponents();

    fixture = TestBed.createComponent(HomeComponent);
    fixture.detectChanges();
    await fixture.whenStable();
  });

  it('saves the values present when restart was requested', () => {
    const component = fixture.componentInstance as unknown as {
      formModel: ReturnType<typeof signal<Config>>;
      save: () => void;
    };

    component.formModel.update((value) => ({
      ...value,
      notifications: { ...value.notifications, enabled: false },
    }));
    component.save();

    // Simulate a config refresh while the running session is still stopping.
    component.formModel.update((value) => ({
      ...value,
      notifications: { ...value.notifications, enabled: true },
    }));
    stop.next();
    stop.complete();

    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ notifications: expect.objectContaining({ enabled: false }) })
    );
  });
});
