const logoModules = import.meta.glob("../../assets/logos/*.svg", {
  eager: true,
  query: "?url",
  import: "default",
});

// Build a map: "nes" -> "/assets/logos/nes.svg" (resolved URL)
const logoMap: Record<string, string> = {};
for (const [path, url] of Object.entries(logoModules)) {
  const name = path.split("/").pop()?.replace(".svg", "");
  if (name) {
    logoMap[name] = url as string;
  }
}

// Regional or split variants of a system use the logo of their base system (ES-DE folder names)
const LOGO_ALIASES: Record<string, string> = {
  megadrivejp: "megadrive",
  megacdjp: "megacd",
  sega32xjp: "sega32x",
  sega32xna: "sega32x",
  saturnjp: "saturn",
  snesna: "snes",
  neogeocdjp: "neogeocd",
  naomigd: "naomi",
  cps: "cps1",
  atarixe: "atari800",
  androidapps: "android",
  androidgames: "android",
};

/** Logo URL for a system folder: its own file, or its base system's for a known variant */
export function logoUrl(name: string): string | undefined {
  const key = name.toLowerCase();
  return logoMap[key] ?? logoMap[LOGO_ALIASES[key] ?? ""];
}

/** Whether there is a logo file for this system (otherwise SystemLogo shows a placeholder) */
export function hasSystemLogo(name: string): boolean {
  return !!logoUrl(name);
}

interface SystemLogoProps {
  name: string;
  size?: number;
  width?: number;
  maxHeight?: number;
  className?: string;
}

export function SystemLogo({ name, size, width, maxHeight, className = "" }: SystemLogoProps) {
  const url = logoUrl(name);
  const effectiveSize = size ?? 20;

  const style: React.CSSProperties = width
    ? { width, height: "auto", maxHeight: maxHeight ?? width }
    : { width: effectiveSize, height: effectiveSize };

  if (!url) {
    return (
      <span
        className={`inline-flex items-center justify-center text-zinc-500 ${className}`}
        style={{ width: width ?? effectiveSize, height: maxHeight ?? effectiveSize, fontSize: (width ?? effectiveSize) * 0.5 }}
      >
        🎮
      </span>
    );
  }

  return (
    <img
      src={url}
      alt={name}
      className={`inline-block object-contain ${className}`}
      style={style}
    />
  );
}
