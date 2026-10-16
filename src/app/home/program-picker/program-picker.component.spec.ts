import { describe, expect, it } from 'vitest';
import { matchPrograms } from './program-picker.component';

const programs = [
  { name: 'obs64.exe', path: 'C:\\Program Files\\obs-studio\\bin\\64bit\\obs64.exe' },
  {
    name: 'FPSAimTrainer-Win64-Shipping.exe',
    path: '/home/u/.local/share/Steam/steamapps/common/FPSAimTrainer/FPSAimTrainer/Binaries/Win64/FPSAimTrainer-Win64-Shipping.exe',
  },
];

describe('matchPrograms', () => {
  it('shows everything without a filter', () => {
    expect(matchPrograms(programs, '  ')).toEqual(programs);
  });

  it('matches the name or the path, ignoring case', () => {
    expect(matchPrograms(programs, 'OBS').map((program) => program.name)).toEqual(['obs64.exe']);
    expect(matchPrograms(programs, 'steamapps').map((program) => program.name)).toEqual([
      'FPSAimTrainer-Win64-Shipping.exe',
    ]);
  });

  it('needs every word', () => {
    expect(matchPrograms(programs, 'steam obs')).toEqual([]);
    expect(matchPrograms(programs, 'aim win64')).toHaveLength(1);
  });
});
