import { SVGProps } from 'react';

type Props = SVGProps<SVGSVGElement> & { size?: number };

function base({ size = 18, ...props }: Props) {
  return {
    width: size,
    height: size,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 1.8,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    'aria-hidden': true,
    ...props,
  };
}

export function IconCaixa(props: Props) {
  return (
    <svg {...base(props)}>
      <path d="M3 7l9-4 9 4-9 4-9-4z" />
      <path d="M3 7v10l9 4 9-4V7" />
      <line x1="12" y1="11" x2="12" y2="21" />
    </svg>
  );
}

export function IconAjuste(props: Props) {
  return (
    <svg {...base(props)}>
      <line x1="4" y1="6" x2="20" y2="6" />
      <circle cx="9" cy="6" r="2" />
      <line x1="4" y1="12" x2="20" y2="12" />
      <circle cx="15" cy="12" r="2" />
      <line x1="4" y1="18" x2="20" y2="18" />
      <circle cx="9" cy="18" r="2" />
    </svg>
  );
}

export function IconChat(props: Props) {
  return (
    <svg {...base(props)}>
      <rect x="3" y="4" width="18" height="12" rx="2" />
      <path d="M7 16l-1 4 4-2" />
    </svg>
  );
}

export function IconRelogio(props: Props) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="9" />
      <line x1="12" y1="12" x2="12" y2="7" />
      <line x1="12" y1="12" x2="16" y2="14" />
    </svg>
  );
}

export function IconUsuarios(props: Props) {
  return (
    <svg {...base(props)}>
      <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
      <circle cx="9" cy="7" r="4" />
      <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
      <path d="M16 3.13a4 4 0 0 1 0 7.75" />
    </svg>
  );
}

export function IconLogout(props: Props) {
  return (
    <svg {...base(props)}>
      <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
      <polyline points="16 17 21 12 16 7" />
      <line x1="21" y1="12" x2="9" y2="12" />
    </svg>
  );
}

export function IconX(props: Props) {
  return (
    <svg {...base(props)}>
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  );
}

export function IconAlerta(props: Props) {
  return (
    <svg {...base(props)}>
      <path d="M12 3l10 18H2L12 3z" />
      <line x1="12" y1="10" x2="12" y2="14" />
      <circle cx="12" cy="17" r="0.5" fill="currentColor" />
    </svg>
  );
}

export function IconCheckCircle(props: Props) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="9" />
      <polyline points="8 12 11 15 16 9" />
    </svg>
  );
}

export function IconInfoCircle(props: Props) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="9" />
      <line x1="12" y1="11" x2="12" y2="16" />
      <circle cx="12" cy="7" r="0.5" fill="currentColor" />
    </svg>
  );
}

export function IconSpinner(props: Props) {
  return (
    <svg {...base(props)} className={`icone-spinner ${props.className ?? ''}`.trim()}>
      <circle cx="12" cy="12" r="9" strokeDasharray="42 30" />
    </svg>
  );
}
