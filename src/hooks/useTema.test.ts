import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useTema } from './useTema';

function mockMatchMedia(prefereEscuro: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockImplementation((query: string) => ({
      matches: query === '(prefers-color-scheme: dark)' && prefereEscuro,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }))
  );
}

describe('useTema', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-tema');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('comeca claro quando o SO prefere claro e nao ha escolha salva', () => {
    mockMatchMedia(false);
    const { result } = renderHook(() => useTema());
    expect(result.current.tema).toBe('claro');
    expect(document.documentElement.getAttribute('data-tema')).toBe('claro');
  });

  it('comeca escuro quando o SO prefere escuro e nao ha escolha salva', () => {
    mockMatchMedia(true);
    const { result } = renderHook(() => useTema());
    expect(result.current.tema).toBe('escuro');
  });

  it('uma escolha salva vence a preferencia do SO', () => {
    mockMatchMedia(true);
    localStorage.setItem('ecoviva-tema', 'claro');
    const { result } = renderHook(() => useTema());
    expect(result.current.tema).toBe('claro');
  });

  it('alternar troca o tema e salva a escolha', () => {
    mockMatchMedia(false);
    const { result } = renderHook(() => useTema());

    act(() => result.current.alternar());
    expect(result.current.tema).toBe('escuro');
    expect(document.documentElement.getAttribute('data-tema')).toBe('escuro');
    expect(localStorage.getItem('ecoviva-tema')).toBe('escuro');

    act(() => result.current.alternar());
    expect(result.current.tema).toBe('claro');
    expect(localStorage.getItem('ecoviva-tema')).toBe('claro');
  });
});
