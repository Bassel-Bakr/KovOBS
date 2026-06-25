import { Service } from '@angular/core';
import { Observable, ReplaySubject } from 'rxjs';
import { Config } from '../models/config';
import { FFmpegDownloadProgress } from '../models/ffmpeg-download-progress';

@Service()
export class EventService {
  messageSubject = new ReplaySubject<string>(1000);
  configSubject = new ReplaySubject<Config>(1000);
  runningSubject = new ReplaySubject<boolean>();
  obsSourcesSubject = new ReplaySubject<string[]>();
  obsRunningSubject = new ReplaySubject<boolean>();
  kovaaksRunningSubject = new ReplaySubject<boolean>();
  aimbeastRunningSubject = new ReplaySubject<boolean>();
  ffmpegDownloadProgressSubject = new ReplaySubject<FFmpegDownloadProgress>();

  messages(): Observable<string> {
    return this.messageSubject.asObservable();
  }

  config(): Observable<Config> {
    return this.configSubject.asObservable();
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

  isAimbeastRunning(): Observable<boolean> {
    return this.aimbeastRunningSubject.asObservable();
  }

  ffmpegDownloadProgress(): Observable<FFmpegDownloadProgress> {
    return this.ffmpegDownloadProgressSubject.asObservable();
  }
}
