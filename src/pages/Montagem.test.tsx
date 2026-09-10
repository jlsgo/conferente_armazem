import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import Montagem from './Montagem';
import * as api from '../lib/api';
import { ToastProvider } from '../lib/toast';
import type { Armazem, Usuario } from '../types';

vi.mock('../lib/api');

const usuario: Usuario = {
  id: 1,
  nome: 'Conferente Teste',
  login: 'conferente',
  armazem_id: 2,
  papel: 'conferente',
  ativo: true,
};

const armazemA4: Armazem = { id: 1, codigo: 'A4', nome: 'Armazem A4' };
const armazemB2: Armazem = { id: 2, codigo: 'B2', nome: 'Armazem B2' };

function renderMontagem() {
  return render(
    <ToastProvider>
      <Montagem usuario={usuario} armazem={armazemB2} armazens={[armazemA4, armazemB2]} />
    </ToastProvider>
  );
}

beforeEach(() => {
  vi.mocked(api.listarMovimentosDoDia).mockResolvedValue([]);
  vi.mocked(api.buscarFechamentoDoDia).mockResolvedValue(null);
  vi.mocked(api.sugestoesDescricao).mockResolvedValue([]);
  vi.mocked(api.criarMovimento).mockResolvedValue({ ok: true });
  vi.mocked(api.buscarTransferenciasPendentes).mockResolvedValue([]);
  vi.mocked(api.buscarTransferenciasRecusadas).mockResolvedValue([]);
});

function selectDeMontagem() {
  return screen
    .getAllByRole('combobox')
    .find((s) => within(s).queryAllByRole('option').some((o) => o.textContent === 'Em caixa'));
}

describe('Montagem - montagem so pra veiculo inteiro', () => {
  it('peca solta (categoria padrao) nao mostra campo de montagem', async () => {
    renderMontagem();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    expect(selectDeMontagem()).toBeUndefined();
  });

  it('categoria de veiculo mostra montagem sem placeholder, comecando em "Em caixa"', async () => {
    const user = userEvent.setup();
    renderMontagem();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    const selectCategoria = screen.getAllByRole('combobox')[0];
    await user.selectOptions(selectCategoria, 'scooter');

    const montagem = selectDeMontagem();
    expect(montagem).toBeDefined();
    expect(montagem).toHaveValue('caixa');
    expect(within(montagem!).getAllByRole('option').map((o) => o.textContent)).toEqual([
      'Em caixa',
      'Montado',
      'Outro',
    ]);
  });

  it('envia montagem null pra peca solta e um valor real pra veiculo', async () => {
    const user = userEvent.setup();
    renderMontagem();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    // Fluxo padrao (tipo=saida, destino=armazem) ja e uma transferencia pro
    // outro armazem - descricao e opcional, montagem continua obrigatoria do
    // lado de quem envia (so a confirmacao de recebimento do outro lado e isenta).
    await user.type(screen.getByPlaceholderText('Descricao (ex: Retrovisor)'), 'Retrovisor');
    // getAllByRole('combobox'): [0] categoria, [1] descricao (tem `list=`,
    // que da role combobox mesmo sendo <input>), [2] condicao - categoria
    // 'peca' nao mostra o select de montagem entre eles.
    const selectCondicao = screen.getAllByRole('combobox')[2];
    await user.selectOptions(selectCondicao, 'boa');

    await user.click(screen.getByRole('button', { name: /^Registrar saida$/ }));

    await waitFor(() => expect(api.criarMovimento).toHaveBeenCalledTimes(1));
    const payload = vi.mocked(api.criarMovimento).mock.calls[0][0];
    expect(payload.itens[0]).toMatchObject({ categoria: 'peca', montagem: null });
  });

  it('no destino "outro armazem" (padrao), o campo "quem retira" e opcional e vira contraparte', async () => {
    const user = userEvent.setup();
    renderMontagem();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    const campoQuemRetira = screen.getByLabelText(/Quem retira\/entrega no destino/i);
    expect(campoQuemRetira).not.toBeRequired();
    await user.type(campoQuemRetira, 'GUSTAVO');

    const selectCondicao = screen.getAllByRole('combobox')[2];
    await user.selectOptions(selectCondicao, 'boa');

    await user.click(screen.getByRole('button', { name: /^Registrar saida$/ }));

    await waitFor(() => expect(api.criarMovimento).toHaveBeenCalledTimes(1));
    const payload = vi.mocked(api.criarMovimento).mock.calls[0][0];
    expect(payload.armazem_destino_id).toBe(armazemA4.id);
    expect(payload.contraparte).toBe('GUSTAVO');
  });
});
