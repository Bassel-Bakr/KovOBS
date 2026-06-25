import { bootstrapApplication } from '@angular/platform-browser';
import { AppComponent } from './app/app.component';
import { appConfig } from './app/app.config';
import { listen } from '@tauri-apps/api/event';
import { EnvironmentInjector } from '@angular/core';
import { EventService } from './app/services/event.service';
import { TauriService } from './app/services/tauri.service';
import { Config } from './app/models/config';
import { FFmpegDownloadProgress } from './app/models/ffmpeg-download-progress';

let injector: EnvironmentInjector | undefined;

void listen<string>('message', (event) => {
  injector?.get(EventService)?.messageSubject.next(event.payload);
});

void listen<Config>('config', (event) => {
  injector?.get(EventService)?.configSubject.next(event.payload);
});

void listen<boolean>('running', (event) => {
  injector?.get(EventService)?.runningSubject.next(event.payload);
});

void listen<string[]>('obs_sources', (event) => {
  injector?.get(EventService)?.obsSourcesSubject.next(event.payload);
});

void listen<boolean>('obs_running', (event) => {
  injector?.get(EventService)?.obsRunningSubject.next(event.payload);
});

void listen<boolean>('kovaaks_running', (event) => {
  injector?.get(EventService)?.kovaaksRunningSubject.next(event.payload);
});

void listen<boolean>('aimbeast_running', (event) => {
  injector?.get(EventService)?.aimbeastRunningSubject.next(event.payload);
});

void listen<FFmpegDownloadProgress>('ffmpeg_download_progress', (event) => {
  injector?.get(EventService)?.ffmpegDownloadProgressSubject.next(event.payload);
});

window.addEventListener('beforeunload', (e) => {
  e.preventDefault();
});

bootstrapApplication(AppComponent, appConfig)
  .then((app) => {
    injector = app.injector;
    const tauriService = injector.get(TauriService);
    // Must be called once to init the app
    tauriService?.init().subscribe();
  })
  .catch((err) => console.error(err));
