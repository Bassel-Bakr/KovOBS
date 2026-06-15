import { bootstrapApplication } from '@angular/platform-browser';
import { AppComponent } from './app/app.component';
import { appConfig } from './app/app.config';
import { listen } from '@tauri-apps/api/event';
import { EnvironmentInjector } from '@angular/core';
import { EventsService } from './app/services/events.service';
import { TauriService } from './app/services/tauri.service';

let injector: EnvironmentInjector | undefined;

bootstrapApplication(AppComponent, appConfig)
  .then((app) => {
    injector = app.injector;
  })
  .catch((err) => console.error(err));

void listen<string>('message', (event) => {
  injector?.get(EventsService)?.subject.next(event.payload);
});

// Wait for ready status
void listen<string>('ready', (event) => {
  injector?.get(TauriService).isReady.next(true);
});
