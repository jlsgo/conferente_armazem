import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import Lancamentos from './Lancamentos';
import * as api from '../lib/api';
import { ToastProvider } from '../lib/toast';
import type { Armazem, Usuario } from '../types';

vi.mock('../lib/api');

const usuario: Usuario = {
  id: 1,
  nome: 'Conferente Teste',
  login: 'conferente',
  armazem_id: 1,
  papel: 'conferente',
  ativo: true,
};

const armazemA4: Armazem = { id: 1, codigo: 'A4', nome: 'Armazem A4' };
const armazemB2: Armazem = { id: 2, codigo: 'B2', nome: 'Armazem B2' };

function renderLancamentos() {
  return render(
    <ToastProvider>
      <Lancamentos usuario={usuario} armazem={armazemA4} armazens={[armazemA4, armazemB2]} />
    </ToastProvider>
  );
}

beforeEach(() => {
  vi.mocked(api.listarMovimentosDoDia).mockResolvedValue([]);
  vi.mocked(api.buscarFechamentoDoDia).mockResolvedValue(null);
  vi.mocked(api.sugestoesDescricao).mockResolvedValue([]);
  vi.mocked(api.criarMovimento).mockResolvedValue({ ok: true });
  vi.mocked(api.verificarRetiradaPendente).mockResolvedValue(null);
  vi.mocked(api.buscarTransferenciasPendentes).mockResolvedValue([]);
  vi.mocked(api.buscarTransferenciasRecusadas).mockResolvedValue([]);
});

describe('Lancamentos - montagem e coleta do item', () => {
  it('nao oferece mais um placeholder em branco pra montagem, e comeca em "Em caixa"', async () => {
    renderLancamentos();
    await waitFor(() => expect(api.listarMovimentosDoDia).toHaveBeenCalled());

    // getAllByRole('combobox') tambem pega o input de descricao (tem `list=`
    // apontando pra um datalist, o que da role combobox mesmo sem ser um
    // <select>) - queryAllByRole (nao lanca em elemento sem opcoes) filtra
    // pra achar so o <select> de montagem de verdade.
    const selects = screen.getAllByRole('combobox');
    const selectMontagem = selects.find((s) =>
      within(s)
        .queryAllByRole('option')
        .some((o) => o.textContent === 'Em caixa')
    );
    expect(selectMontagem).toBeDefined();

    const opcoes = within(selectMontagem!)
      .getAllByRole('option')
      .map((o) => o.textContent);
    expect(opcoes).toEqual(['Em caixa', 'Montado', 'Outro']);
    expect(selectMontagem).toHaveValue('caixa');
  });

  it('exige coleta pra registrar uma saida, e envia o pedido sem quem_retirou', async () => {
    const user = userEvent.setup();
    renderLancamentos();
    await waitFor(() => expect(api.listarMovimentosDoDia).toHaveBeenCalled());

    await user.type(screen.getByPlaceholderText('Ex: 3932'), '4001');

    const campoDescricaoItem = screen.getByPlaceholderText('Detalhe do item (ex: HE-15 GREEN)');
    expect(campoDescricaoItem).toBeRequired();
    await user.type(campoDescricaoItem, 'HE-15 CARBON');

    const campoColeta = screen.getByLabelText(/Coleta \(transportadora/i);
    expect(campoColeta).toBeRequired();
    await user.type(campoColeta, 'DISK&TENHA');

    await user.click(screen.getByRole('button', { name: /^Registrar saida$/ }));

    await waitFor(() => expect(api.criarMovimento).toHaveBeenCalledTimes(1));
    const payload = vi.mocked(api.criarMovimento).mock.calls[0][0];
    expect(payload.contraparte).toBe('DISK&TENHA');
    expect(payload.quem_retirou).toBeUndefined();
    expect(payload.itens[0]).toMatchObject({ descricao: 'HE-15 CARBON', montagem: 'caixa' });
  });
});
