const HEX = /^#[0-9a-f]{6}$/i;

function channel(hex: string, at: number): number {
  const v = Number.parseInt(hex.slice(at, at + 2), 16) / 255;
  return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  return 0.2126 * channel(hex, 1) + 0.7152 * channel(hex, 3) + 0.0722 * channel(hex, 5);
}

export function contrast(a: string, b: string): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

function mix(hex: string, target: string, amount: number): string {
  const part = (at: number) => {
    const from = Number.parseInt(hex.slice(at, at + 2), 16);
    const to = Number.parseInt(target.slice(at, at + 2), 16);
    return Math.round(from + (to - from) * amount)
      .toString(16)
      .padStart(2, '0');
  };
  return `#${part(1)}${part(3)}${part(5)}`;
}

/** A team colour readable on `ground` (contrast 3:1): ESPN's colour, lightened on a dark ground and darkened on a light one; the fallback when the value is not a plain hex colour. */
export function readable(colour: string, ground: string, fallback = '#8a8f98'): string {
  let result = HEX.test(colour) ? colour.toLowerCase() : fallback;
  const toward = luminance(ground) < 0.4 ? '#ffffff' : '#000000';
  for (let i = 0; i < 12 && contrast(result, ground) < 3; i += 1)
    result = mix(result, toward, 0.15);
  return result;
}
