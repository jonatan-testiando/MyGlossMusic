import type { JSX } from "solid-js";

type P = { size?: number; class?: string };

const svg = (path: JSX.Element, fill = true) => (p: P) =>
  (
    <svg
      width={p.size ?? 20}
      height={p.size ?? 20}
      viewBox="0 0 24 24"
      fill={fill ? "currentColor" : "none"}
      stroke={fill ? "none" : "currentColor"}
      stroke-width={fill ? 0 : 1.8}
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
