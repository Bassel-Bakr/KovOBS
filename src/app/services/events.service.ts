import { Service } from '@angular/core';
import { Observable, Subject } from 'rxjs';

@Service()
export class EventsService {
  messageSubject = new Subject<string>();
  runningSubject = new Subject<boolean>();
  obsSourcesSubject = new Subject<string[]>();
  obsRunningSubject = new Subject<boolean>();
  kovaaksRunningSubject = new Subject<boolean>();

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
