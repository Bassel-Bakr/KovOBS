import { Service } from '@angular/core';
import { Observable, Subject } from 'rxjs';

@Service()
export class EventsService {
  subject = new Subject<string>();

  message(): Observable<string> {
    return this.subject.asObservable();
  }
}
