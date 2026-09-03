import type { JSX } from "solid-js";

type P = { size?: number; class?: string };

/**
 * Grosor de trazo que se ve IGUAL a cualquier tamanio.
 *
 * El `viewBox` es siempre de 24, asi que un `stroke-width` fijo adelgaza al
 * reducir el icono: 1,8 en un icono de 12 px se pinta a 0,9 px reales, y sobre
 * un fondo claro eso desaparece. Los iconos de la barra de titulo (12-15 px)
 * eran justo los que se perdian, mientras que el de minimizar — una linea
 * larga y sola — aguantaba.
 *
 * Escalando el grosor con el tamanio, todos se pintan a ~1,5 px reales.
 */
const trazo = (size: number) => Math.min(3.2, (1.8 * 20) / size);

const svg = (path: JSX.Element, fill = true) => (p: P) =>
  (
    <svg
      width={p.size ?? 20}
      height={p.size ?? 20}
      viewBox="0 0 24 24"
      fill={fill ? "currentColor" : "none"}
      stroke={fill ? "none" : "currentColor"}
      stroke-width={fill ? 0 : trazo(p.size ?? 20)}
      stroke-linecap="round"
      stroke-linejoin="round"
      class={p.class}
      aria-hidden="true"
    >
      {path}
    </svg>
  );

export const Play = svg(<path d="M8 5.14v13.72a1 1 0 0 0 1.54.84l10.4-6.86a1 1 0 0 0 0-1.68L9.54 4.3A1 1 0 0 0 8 5.14Z" />);
export const Pause = svg(<path d="M7 4h3.5v16H7zM13.5 4H17v16h-3.5z" />);
export const Prev = svg(<path d="M7 5h2.2v14H7zm12 .9v12.2a1 1 0 0 1-1.55.83l-9-6.1a1 1 0 0 1 0-1.66l9-6.1A1 1 0 0 1 19 5.9Z" />);
export const Next = svg(<path d="M14.8 5H17v14h-2.2zM5 5.9v12.2a1 1 0 0 0 1.55.83l9-6.1a1 1 0 0 0 0-1.66l-9-6.1A1 1 0 0 0 5 5.9Z" />);

export const Shuffle = svg(
  <>
    <path d="M16 3h5v5" />
    <path d="M4 20 21 3" />
    <path d="M21 16v5h-5" />
    <path d="m15 15 6 6" />
    <path d="M4 4l5 5" />
  </>,
  false,
);

export const Repeat = svg(
  <>
    <path d="m17 2 4 4-4 4" />
    <path d="M3 11v-1a4 4 0 0 1 4-4h14" />
    <path d="m7 22-4-4 4-4" />
    <path d="M21 13v1a4 4 0 0 1-4 4H3" />
  </>,
  false,
);

export const RepeatOne = svg(
  <>
    <path d="m17 2 4 4-4 4" />
    <path d="M3 11v-1a4 4 0 0 1 4-4h14" />
    <path d="m7 22-4-4 4-4" />
    <path d="M21 13v1a4 4 0 0 1-4 4H3" />
    <path d="M11 10h1v4" />
  </>,
  false,
);

export const Volume = svg(
  <>
    <path d="M11 5 6 9H3v6h3l5 4z" />
    <path d="M16 9a4 4 0 0 1 0 6" />
    <path d="M19 6a8 8 0 0 1 0 12" />
  </>,
  false,
);

export const VolumeMute = svg(
  <>
    <path d="M11 5 6 9H3v6h3l5 4z" />
    <path d="m17 9 5 6M22 9l-5 6" />
  </>,
  false,
);

export const Search = svg(
  <>
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.5-3.5" />
  </>,
  false,
);

export const Home = svg(
  <>
    <path d="M3 10.5 12 3l9 7.5" />
    <path d="M5.5 9.5V20h13V9.5" />
  </>,
  false,
);

export const Stethoscope = svg(
  <>
    <path d="M5 3v6a4 4 0 0 0 8 0V3" />
    <path d="M5 3h2M11 3h2" />
    <path d="M9 13v2a5 5 0 0 0 10 0v-2" />
    <circle cx="19" cy="10" r="2" />
  </>,
  false,
);

export const Minimize = svg(<path d="M5 12h14" />, false);
export const Maximize = svg(<rect x="5.5" y="5.5" width="13" height="13" rx="1.5" />, false);
export const Close = svg(<path d="m6 6 12 12M18 6 6 18" />, false);
export const Music = svg(
  <>
    <path d="M9 18V5l11-2v13" />
    <circle cx="6" cy="18" r="3" />
    <circle cx="17" cy="16" r="3" />
  </>,
  false,
);

export const Heart = svg(
  <path d="M12 20.3 4.6 13a4.7 4.7 0 0 1 6.6-6.7l.8.8.8-.8A4.7 4.7 0 0 1 19.4 13Z" />,
  false,
);
export const HeartFilled = svg(
  <path d="M12 20.3 4.6 13a4.7 4.7 0 0 1 6.6-6.7l.8.8.8-.8A4.7 4.7 0 0 1 19.4 13Z" />,
);
export const Library = svg(
  <>
    <path d="M4 4v16M9 4v16" />
    <path d="m14 5 5 15" />
  </>,
  false,
);

export const Lyrics = svg(
  <>
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    <line x1="8" y1="9" x2="16" y2="9" />
    <line x1="8" y1="13" x2="14" y2="13" />
  </>,
  false,
);

export const More = svg(
  <>
    <circle cx="12" cy="12" r="1.5" />
    <circle cx="12" cy="5" r="1.5" />
    <circle cx="12" cy="19" r="1.5" />
  </>,
);

export const Menu = svg(
  <>
    <line x1="3" y1="12" x2="21" y2="12" />
    <line x1="3" y1="6" x2="21" y2="6" />
    <line x1="3" y1="18" x2="21" y2="18" />
  </>,
  false,
);

export const Compass = svg(
  <>
    <circle cx="12" cy="12" r="10" />
    <polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76" />
  </>,
  false,
);

export const ThumbsUp = svg(
  <path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3" />,
  false,
);

export const ThumbsDown = svg(
  <path d="M10 15v4a3 3 0 0 0 3 3l4-9V2H5.72a2 2 0 0 0-2 1.7l-1.38 9a2 2 0 0 0 2 2.3zm7-13h3a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-3" />,
  false,
);

export const ChevronUp = svg(<path d="m18 15-6-6-6 6" />, false);
export const ChevronDown = svg(<path d="m6 9 6 6 6-6" />, false);

export const Pin = svg(
  <>
    <line x1="12" y1="17" x2="12" y2="22" />
    <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a1 1 0 0 0 0-2H8a1 1 0 0 0 0 2h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z" />
  </>,
  false,
);

export const Plus = svg(
  <>
    <line x1="12" y1="5" x2="12" y2="19" />
    <line x1="5" y1="12" x2="19" y2="12" />
  </>,
  false,
);

export const ChevronLeft = svg(<path d="m15 18-6-6 6-6" />, false);
export const ChevronRight = svg(<path d="m9 18 6-6-6-6" />, false);
export const Refresh = svg(
  <>
    <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
  </>,
  false,
);

export const Trash = svg(
  <>
    <path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
    <path d="M6 7v12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7" />
    <path d="M10 11v6M14 11v6" />
  </>,
  false,
);
