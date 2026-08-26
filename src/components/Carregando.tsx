import { IconSpinner } from './Icon';

export default function Carregando({ texto = 'Carregando...' }: { texto?: string }) {
  return (
    <p className="carregando">
      <IconSpinner size={16} />
      {texto}
    </p>
  );
}
