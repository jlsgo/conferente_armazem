import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import Sac from './Sac';
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

function renderSac() {
  return render(
    <ToastProvider>
      <Sac usuario={usuario} armazem={armazemA4} armazens={[armazemA4, armazemB2]} />
    </ToastProvider>
  );
}

beforeEach(() => {
  vi.mocked(api.listarMovimentosDoDia).mockResolvedValue([]);
  vi.mocked(api.buscarFechamentoDoDia).mockResolvedValue(null);
  vi.mocked(api.sugestoesDescricao).mockResolvedValue([]);
  vi.mocked(api.sugestoesContraparte).mockResolvedValue([]);
  vi.mocked(api.sugestoesRazaoSocial).mockResolvedValue([]);
  vi.mocked(api.criarMovimento).mockResolvedValue({ ok: true });
  // Sub-componentes independentes do formulario (aviso de transferencia
  // chegando/recusada) - sem isso eles tentam chamar a API real e poluem a
  // tela com toasts de erro que atrapalham os asserts sobre o formulario.
  vi.mocked(api.buscarTransferenciasPendentes).mockResolvedValue([]);
  vi.mocked(api.buscarTransferenciasRecusadas).mockResolvedValue([]);
});

describe('Sac - motivo de saida', () => {
  it('nao oferece mais "Entregue ao cliente" como opcao de motivo', async () => {
    renderSac();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    const select = screen.getByRole('combobox', { name: /motivo da saida/i });
    const opcoes = within(select)
      .getAllByRole('option')
      .map((o) => o.textContent);
    expect(opcoes).not.toContain('Entregue ao cliente');
    expect(opcoes).toEqual(
      expect.arrayContaining(['Descarte (sucata)', 'Garantia', 'Venda', 'Outro'])
    );
  });

  it('nao exige valor quando o motivo da saida e descarte', async () => {
    const user = userEvent.setup();
    renderSac();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    await user.type(screen.getByPlaceholderText('Numero do protocolo'), '123');
    await user.selectOptions(
      screen.getByRole('combobox', { name: /motivo da saida/i }),
      'descarte'
    );
    expect(screen.queryByLabelText(/^valor/i)).not.toBeInTheDocument();

    await user.type(
      screen.getByPlaceholderText('Descricao da peça (ex: Retrovisor)'),
      'Retrovisor quebrado'
    );
    await user.click(screen.getByRole('button', { name: /registrar/i }));

    await waitFor(() => expect(api.criarMovimento).toHaveBeenCalledTimes(1));
    const payload = vi.mocked(api.criarMovimento).mock.calls[0][0];
    expect(payload.motivo).toBe('descarte');
    expect(payload.valor_centavos).toBeNull();
  });

  it('exige valor quando o motivo da saida e garantia', async () => {
    const user = userEvent.setup();
    renderSac();
    await waitFor(() => expect(screen.queryByText('Carregando...')).not.toBeInTheDocument());

    await user.type(screen.getByPlaceholderText('Numero do protocolo'), '124');
    await user.selectOptions(
      screen.getByRole('combobox', { name: /motivo da saida/i }),
      'garantia'
    );

    // O campo so aparece (e e obrigatorio via HTML5) quando o motivo exige
    // valor - a validacao nativa do browser ja bloqueia o submit vazio antes
    // do handler customizado rodar, entao o teste relevante e o campo existir
    // e ser obrigatorio, e o fluxo completar quando preenchido.
    const campoValor = screen.getByLabelText(/^valor/i);
    expect(campoValor).toBeRequired();

    await user.type(
      screen.getByPlaceholderText('Descricao da peça (ex: Retrovisor)'),
      'Bateria trocada em garantia'
    );
    // Digitado com ponto, nao virgula: o input e type="number", que em jsdom
    // (assim como em varios browsers) so aceita ponto como separador decimal -
    // o `replace(',', '.')` no handler e so uma rede de seguranca.
    await user.type(campoValor, '150.00');
    await user.click(screen.getByRole('button', { name: /registrar/i }));

    await waitFor(() => expect(api.criarMovimento).toHaveBeenCalledTimes(1));
    const payload = vi.mocked(api.criarMovimento).mock.calls[0][0];
    expect(payload.motivo).toBe('garantia');
    expect(payload.valor_centavos).toBe(15000);
  });
});
