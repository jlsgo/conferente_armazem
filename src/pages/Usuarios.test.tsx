import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import Usuarios from './Usuarios';
import * as api from '../lib/api';
import type { Armazem } from '../types';

vi.mock('../lib/api');

const armazemA4: Armazem = { id: 1, codigo: 'A4', nome: 'Armazem A4' };

beforeEach(() => {
  vi.mocked(api.listarUsuarios).mockResolvedValue([]);
});

describe('Usuarios - verificar integridade', () => {
  it('mostra cadeia intacta quando o backend retorna null', async () => {
    const user = userEvent.setup();
    vi.mocked(api.verificarIntegridade).mockResolvedValue({ ok: true, quebra: null });
    render(<Usuarios armazens={[armazemA4]} />);
    await waitFor(() => expect(api.listarUsuarios).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /verificar integridade/i }));

    await waitFor(() =>
      expect(screen.getByText(/cadeia intacta/i)).toBeInTheDocument()
    );
  });

  it('mostra a quebra encontrada quando o backend retorna uma', async () => {
    const user = userEvent.setup();
    vi.mocked(api.verificarIntegridade).mockResolvedValue({
      ok: true,
      quebra: { movimento_id: 42, numero_pedido: '3893' },
    });
    render(<Usuarios armazens={[armazemA4]} />);
    await waitFor(() => expect(api.listarUsuarios).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /verificar integridade/i }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('42')
    );
    expect(screen.getByRole('alert')).toHaveTextContent('3893');
  });

  it('so um gestor pode chamar - mostra o erro do backend se rejeitado', async () => {
    const user = userEvent.setup();
    vi.mocked(api.verificarIntegridade).mockResolvedValue({
      ok: false,
      error: 'Somente um gestor pode verificar a integridade da cadeia de auditoria.',
    });
    render(<Usuarios armazens={[armazemA4]} />);
    await waitFor(() => expect(api.listarUsuarios).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /verificar integridade/i }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('Somente um gestor')
    );
  });
});
