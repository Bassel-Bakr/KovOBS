import { bootstrapApplication } from '@angular/platform-browser';
import { AppComponent } from './app/app.component';
import { appConfig } from './app/app.config';
import { listen } from '@tauri-apps/api/event';
import { EnvironmentInjector } from '@angular/core';
import { EventsService } from './app/services/events.service';
import { TauriService } from './app/services/tauri.service';

let injector: EnvironmentInjector | undefined;

void listen<string>('message', (event) => {
  injector?.get(EventsService)?.messageSubject.next(event.payload);
});

void listen<boolean>('running', (event) => {
  injector?.get(EventsService)?.runningSubject.next(event.payload);
});

void listen<string[]>('obs_sources', (event) => {
  injector?.get(EventsService)?.obsSourcesSubject.next(event.payload);
});

bootstrapApplication(AppComponent, appConfig)
  .then((app) => {
    injector = app.injector;
    const tauriService = injector.get(TauriService);
    // Must be called once to init the app
    tauriService?.init().subscribe();
  })
  .catch((err) => console.error(err));
