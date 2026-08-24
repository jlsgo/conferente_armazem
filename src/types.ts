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

export type Categoria = 'scooter' | 'triciclo' | 'patinete' | 'peca';
export type TipoMovimento = 'entrada' | 'saida';
export type Fluxo = 'saida_armazem' | 'peca_montagem' | 'sac';
export type Montagem = 'montado' | 'caixa';
export type Condicao = 'boa' | 'defeito' | 'sucata';

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
  usuario_id: number;
  numero_pedido?: string | null;
  codigo_rastreio?: string | null;
  contraparte?: string | null;
  quem_retirou?: string | null;
  motivo?: string | null;
  valor_centavos?: number | null;
  observacoes?: string | null;
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
}

export interface Movimento {
  id: number;
  numero: number;
  armazem_id: number;
  fluxo: Fluxo;
  tipo: TipoMovimento;
  data: string;
  hora: string;
  turno: string;
  usuario_id: number;
  usuario_nome: string;
  numero_pedido: string | null;
  contraparte: string | null;
  quem_retirou: string | null;
  status: string;
  itens: MovimentoItem[];
}
