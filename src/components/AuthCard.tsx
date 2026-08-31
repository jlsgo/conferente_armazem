import { FormEvent, ReactNode } from 'react';
import logoEcoviva from '../assets/ecoviva-logo.png';
import { useCliquesSecretos } from '../hooks/useCliquesSecretos';
import CobrinhaSecreta from './CobrinhaSecreta';

interface Props {
  titulo: string;
  subtitulo?: string;
  versao?: string;
  erro: string;
  enviando: boolean;
  textoBotao: string;
  textoBotaoEnviando: string;
  onSubmit: (e: FormEvent) => void;
  children: ReactNode;
}

/**
 * Casca compartilhada entre Login e Setup (logo, cartao centralizado,
 * titulo/subtitulo, mensagem de erro, botao com estado "enviando") - as duas
 * telas eram quase identicas nesse entorno, so diferindo nos campos do meio
 * (children) e nos textos.
 */
export default function AuthCard({
  titulo,
  subtitulo,
  versao,
  erro,
  enviando,
  textoBotao,
  textoBotaoEnviando,
  onSubmit,
  children,
}: Props) {
  const cobrinha = useCliquesSecretos();

  return (
    <div className="tela-centralizada">
      <form className="cartao" onSubmit={onSubmit}>
        <img
          src={logoEcoviva}
          alt="Ecoviva"
          className="logo-ecoviva"
          onClick={cobrinha.registrarClique}
        />
        <h1>{titulo}</h1>
        {subtitulo && <p className="subtitulo">{subtitulo}</p>}
        {versao && <span className="pilula-versao">v{versao}</span>}

        {children}

        {erro && <p className="erro">{erro}</p>}

        <button type="submit" disabled={enviando}>
          {enviando ? textoBotaoEnviando : textoBotao}
        </button>
      </form>
      {cobrinha.ativo && <CobrinhaSecreta onFechar={cobrinha.fechar} />}
    </div>
  );
}
