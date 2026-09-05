export type LogLevel = 'info' | 'clip' | 'error';

export type LogEntry = {
  id: number;
  time: string;
  text: string;
  level: LogLevel;
};
