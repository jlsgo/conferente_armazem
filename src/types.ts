export interface Armazem {
  id: number;
  codigo: string;
  nome: string;
}

export interface AppStatus {
  precisa_configurar_primeiro_usuario: boolean;
  armazens: Armazem[];
  versao: string;
}

export interface Usuario {
  id: number;
  nome: string;
  login: string;
  armazem_id: number | null;
  papel: 'conferente' | 'gestor';
  ativo: boolean;
}

export type Categoria = 'scooter' | 'triciclo' | 'patinete' | 'peca' | 'outro';
export type TipoMovimento = 'entrada' | 'saida';
export type Fluxo = 'saida_armazem' | 'peca_montagem' | 'sac' | 'reparo_externo';
/** Variante do fechamento diario/impressao - mesmos fluxos, nomes diferentes
 * (usada por `FechamentoImpressao.tsx`/`exportFechamento.ts`, nao vem do banco). */
export type VarianteFechamento = 'armazem' | 'montagem' | 'sac' | 'reparo_externo';
export type Montagem = 'montado' | 'caixa';
export type Condicao = 'boa' | 'defeito' | 'sucata' | 'outro';

export interface MovimentoItemInput {
  categoria: Categoria;
  descricao?: string | null;
  montagem?: Montagem | null;
  condicao?: Condicao | null;
  quantidade: number;
  observacao?: string | null;
  /** Codigo/serie do componente (bateria, motor, modulo) - obrigatorio
   * quando `fluxo === 'reparo_externo'`, usado pra casar a saida pro
   * tecnico externo com a entrada de retorno. */
  codigo_componente?: string | null;
}

export interface NovoMovimento {
  armazem_id: number;
  armazem_destino_id?: number | null;
  fluxo: Fluxo;
  tipo: TipoMovimento;
  data: string;
  hora: string;
  turno: 'diurno' | 'noturno';
  numero_pedido?: string | null;
  codigo_rastreio?: string | null;
  contraparte?: string | null;
  quem_retirou?: string | null;
  motivo?: string | null;
  valor_centavos?: number | null;
  observacoes?: string | null;
  /** So relevante pra saida_armazem/saida - default true nas outras telas. */
  retirada_completa?: boolean;
  itens: MovimentoItemInput[];
}

export interface MovimentoItem {
  id: number;
  categoria: Categoria;
  descricao: string | null;
  montagem: Montagem | null;
  condicao: Condicao | null;
  quantidade: number;
  observacao: string | null;
  quantidade_enviada: number | null;
  codigo_componente: string | null;
}

export interface Movimento {
  id: number;
  numero: number;
  armazem_id: number;
  armazem_destino_id: number | null;
  fluxo: Fluxo;
  tipo: TipoMovimento;
  data: string;
  hora: string;
  turno: string;
  usuario_id: number;
  usuario_nome: string;
  numero_pedido: string | null;
  codigo_rastreio: string | null;
  contraparte: string | null;
  quem_retirou: string | null;
  motivo: string | null;
  valor_centavos: number | null;
  observacoes: string | null;
  status: string;
  estornado_de: number | null;
  recebido_de_armazem_codigo: string | null;
  recebido_de_id_origem: number | null;
  retirada_completa: boolean;
  hash_integridade: string;
  itens: MovimentoItem[];
}

export interface ResultadoHistorico {
  movimentos: Movimento[];
  tem_mais: boolean;
}

export interface TransferenciaPendente {
  armazem_origem_codigo: string;
  id_origem: number;
  fluxo: Fluxo;
  data: string;
  hora: string;
  armazem_destino_codigo: string | null;
  numero_pedido: string | null;
  observacoes: string | null;
  itens: MovimentoItem[];
}

/** Uma transferencia que EU enviei e que o outro armazem recusou - ver
 * `recusarRecebimento`/`buscarTransferenciasRecusadas`. `meu_movimento_id` e
 * o id do MEU lancamento original (nao um id do Turso) - usado pra abrir e
 * estornar ele na minha propria lista. */
export interface TransferenciaRecusada {
  armazem_que_recusou_codigo: string;
  meu_movimento_id: number;
  fluxo: Fluxo;
  data: string;
  hora: string;
  numero_pedido: string | null;
  justificativa: string | null;
  itens: MovimentoItem[];
}

/** Um item enviado pra reparo externo que ainda nao voltou (nenhuma entrada
 * registrada ainda com o mesmo `codigo_componente` nesse armazem/fluxo). */
export interface ReparoPendente {
  movimento_id: number;
  item_id: number;
  codigo_componente: string;
  categoria: Categoria;
  descricao: string | null;
  quantidade: number;
  contraparte: string | null;
  data: string;
  hora: string;
}

/** Um reparo externo que saiu e voltou consertado (`condicao 'boa'` na
 * entrada) - usado pelo relatorio de pagamento por quinzena do tecnico. */
export interface ReparoConcluido {
  movimento_id_saida: number;
  movimento_id_entrada: number;
  item_id_saida: number;
  codigo_componente: string;
  categoria: Categoria;
  descricao: string | null;
  quantidade: number;
  contraparte: string | null;
  data_saida: string;
  hora_saida: string;
  data_entrada: string;
  hora_entrada: string;
  observacao_entrada: string | null;
}

export interface Fechamento {
  id: number;
  armazem_id: number;
  fluxo: Fluxo;
  data: string;
  usuario_id: number;
  usuario_nome: string;
  total_itens: number;
  hash_integridade: string;
  criado_em: string;
  total_estornado: number;
  total_liquido: number;
}

export interface StatusSincronizacao {
  pendentes: number;
  com_erro: number;
  ultimo_erro: string | null;
}

export interface NovoUsuarioInput {
  nome: string;
  login: string;
  senha: string;
  armazem_id: number | null;
  papel: 'conferente' | 'gestor';
}
