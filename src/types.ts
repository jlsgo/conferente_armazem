export interface Armazem {
  id: number;
  codigo: string;
  nome: string;
}

export interface AppStatus {
  precisa_configurar_primeiro_usuario: boolean;
  armazens: Armazem[];
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
export type Fluxo = 'saida_armazem' | 'peca_montagem' | 'sac';
export type Montagem = 'montado' | 'caixa';
export type Condicao = 'boa' | 'defeito' | 'sucata' | 'outro';

export interface MovimentoItemInput {
  categoria: Categoria;
  descricao?: string | null;
  montagem?: Montagem | null;
  condicao?: Condicao | null;
  quantidade: number;
  observacao?: string | null;
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

export interface TransferenciaPendente {
  armazem_origem_codigo: string;
  id_origem: number;
  fluxo: Fluxo;
  data: string;
  hora: string;
  armazem_destino_codigo: string | null;
  itens: MovimentoItem[];
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
