import { Service } from '@angular/core';
import { Observable, Subject } from 'rxjs';

@Service()
export class EventsService {
  messageSubject = new Subject<string>();
  runningSubject = new Subject<boolean>();
  obsSourcesSubject = new Subject<string[]>();

  messages(): Observable<string> {
    return this.messageSubject.asObservable();
  }

  isRunning(): Observable<boolean> {
    return this.runningSubject.asObservable();
  }

  obsSources(): Observable<string[]> {
    return this.obsSourcesSubject.asObservable();
  }
}
