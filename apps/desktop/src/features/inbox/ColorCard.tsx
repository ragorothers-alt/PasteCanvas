interface ColorCardProps {
  colorText: string;
}

/**
 * ColorCard — displays a color swatch with its value.
 * Used in Inbox and Canvas for color-type items.
 */
export function ColorCard({ colorText }: ColorCardProps) {
  const trimmed = colorText.trim();

  // Determine the CSS color value to display
  const cssColor = normalizeCssColor(trimmed);

  return (
    <div className="color-card">
      <div
        className="color-swatch"
        style={{ backgroundColor: cssColor }}
        title={trimmed}
      />
      <div className="color-info">
        <span className="color-value">{trimmed}</span>
        <span className="color-format">{detectFormat(trimmed)}</span>
      </div>
    </div>
  );
}

function normalizeCssColor(text: string): string {
  // Most color formats are valid CSS directly
  if (text.startsWith("#") || text.startsWith("rgb") || text.startsWith("hsl")) {
    return text;
  }
  // Named colors are valid CSS
  return text;
}

function detectFormat(text: string): string {
  if (text.startsWith("#")) {
    const len = text.length - 1;
    if (len === 3) return "HEX3";
    if (len === 6) return "HEX";
    if (len === 8) return "HEX8";
    return "HEX";
  }
  if (text.startsWith("rgba")) return "RGBA";
  if (text.startsWith("rgb")) return "RGB";
  if (text.startsWith("hsl")) return "HSL";
  return "Named";
}
