import { Service } from '@angular/core';
import { Observable, ReplaySubject } from 'rxjs';

@Service()
export class EventsService {
  messageSubject = new ReplaySubject<string>(1000);
  runningSubject = new ReplaySubject<boolean>();
  obsSourcesSubject = new ReplaySubject<string[]>();
  obsRunningSubject = new ReplaySubject<boolean>();
  kovaaksRunningSubject = new ReplaySubject<boolean>();

  messages(): Observable<string> {
    return this.messageSubject.asObservable();
  }

  isRunning(): Observable<boolean> {
    return this.runningSubject.asObservable();
  }

  obsSources(): Observable<string[]> {
    return this.obsSourcesSubject.asObservable();
  }

  isObsRunning(): Observable<boolean> {
    return this.obsRunningSubject.asObservable();
  }

  isKovaaksRunning(): Observable<boolean> {
    return this.kovaaksRunningSubject.asObservable();
  }
}
