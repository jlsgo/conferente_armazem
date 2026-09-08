import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import AuthCard from './AuthCard';

describe('AuthCard', () => {
  it('renderiza titulo, subtitulo e aciona onSubmit ao enviar o formulario', async () => {
    const onSubmit = vi.fn((e) => e.preventDefault());
    render(
      <AuthCard
        titulo="Entrar"
        subtitulo="Armazem A4"
        erro=""
        enviando={false}
        textoBotao="Entrar"
        textoBotaoEnviando="Entrando..."
        onSubmit={onSubmit}
      >
        <input aria-label="login" />
      </AuthCard>,
    );

    expect(screen.getByRole('heading', { name: 'Entrar' })).toBeInTheDocument();
    expect(screen.getByText('Armazem A4')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Entrar' }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it('mostra o texto de "enviando" e desabilita o botao quando enviando=true', () => {
    render(
      <AuthCard
        titulo="Entrar"
        erro=""
        enviando={true}
        textoBotao="Entrar"
        textoBotaoEnviando="Entrando..."
        onSubmit={vi.fn()}
      >
        <input aria-label="login" />
      </AuthCard>,
    );

    const botao = screen.getByRole('button', { name: 'Entrando...' });
    expect(botao).toBeDisabled();
  });

  it('mostra a mensagem de erro quando presente', () => {
    render(
      <AuthCard
        titulo="Entrar"
        erro="Credenciais invalidas"
        enviando={false}
        textoBotao="Entrar"
        textoBotaoEnviando="Entrando..."
        onSubmit={vi.fn()}
      >
        <input aria-label="login" />
      </AuthCard>,
    );

    expect(screen.getByRole('alert')).toHaveTextContent('Credenciais invalidas');
  });
});
