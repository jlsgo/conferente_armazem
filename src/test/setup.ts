import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockRejectedValue(new Error('invoke() nao mockado neste teste')),
}));

// Sem `test.globals` no vitest.config.ts, o auto-cleanup implicito do
// testing-library (que depende de um `afterEach` global) nao dispara -
// registrado explicitamente aqui pra cada teste comecar com o DOM limpo.
afterEach(() => {
  cleanup();
});
