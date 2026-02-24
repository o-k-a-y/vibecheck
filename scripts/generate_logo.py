#!/usr/bin/env python3
"""Generate .github/assets/logo.svg — vibecheck wordmark with model-color gradient.

Run from the repo root:
    python3 scripts/generate_logo.py
"""

FONT = "ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace"
BG   = "#0d1117"
W, H = 760, 215

# Model colors match vibecheck-core/src/colors.rs svg_color()
# Order matches the gradient left-to-right for visual alignment.
models = [
    ("Claude",  "#d2a8ff"),
    ("Gemini",  "#79c0ff"),
    ("Copilot", "#39c5cf"),
    ("GPT",     "#7ee787"),
    ("Human",   "#e3b341"),
]

# Gradient uses the same model colors in spectral order (smooth transitions).
grad_stops = [
    ( 0, "#d2a8ff"),   # Claude  — lavender
    (25, "#79c0ff"),   # Gemini  — blue
    (50, "#39c5cf"),   # Copilot — teal
    (75, "#7ee787"),   # GPT     — green
    (100,"#e3b341"),   # Human   — amber
]

FS   = 76
WORD = "vibecheck"

# Anchor gradient tightly to the rendered text width so the full spectrum shows.
text_w = len(WORD) * FS * 0.601
x1 = (W - text_w) / 2
x2 = x1 + text_w

stop_xml = "\n      ".join(
    f'<stop offset="{p}%" stop-color="{c}"/>' for p, c in grad_stops
)

n = len(models)
pill_cx = [(W / (n + 1)) * (i + 1) for i in range(n)]
dot_cy  = 175
label_y = 195
sep_y   = 152

svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}">
  <defs>
    <linearGradient id="g" gradientUnits="userSpaceOnUse" x1="{x1:.1f}" y1="0" x2="{x2:.1f}" y2="0">
      {stop_xml}
    </linearGradient>
    <filter id="glow" x="-20%" y="-80%" width="140%" height="260%">
      <feGaussianBlur in="SourceGraphic" stdDeviation="9" result="blur"/>
      <feMerge>
        <feMergeNode in="blur"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
  </defs>

  <rect width="{W}" height="{H}" fill="{BG}" rx="14"/>

  <!-- main wordmark — glow pass -->
  <text x="50%" y="108" text-anchor="middle"
        font-family="{FONT}" font-size="{FS}px" font-weight="bold"
        fill="url(#g)" opacity="0.45" filter="url(#glow)">vibecheck</text>
  <!-- main wordmark — crisp pass -->
  <text x="50%" y="108" text-anchor="middle"
        font-family="{FONT}" font-size="{FS}px" font-weight="bold"
        fill="url(#g)">vibecheck</text>

  <!-- tagline -->
  <text x="50%" y="134" text-anchor="middle"
        font-family="{FONT}" font-size="12.5px" fill="#8b949e" letter-spacing="0.5">detect the AI behind the code</text>

  <!-- separator -->
  <line x1="{W*0.08:.0f}" y1="{sep_y}" x2="{W*0.92:.0f}" y2="{sep_y}"
        stroke="#21262d" stroke-width="1"/>
"""

for (name, color), cx in zip(models, pill_cx):
    svg += f"""
  <circle cx="{cx:.1f}" cy="{dot_cy}" r="5" fill="{color}" opacity="0.9"/>
  <text x="{cx:.1f}" y="{label_y}" text-anchor="middle"
        font-family="{FONT}" font-size="11px" fill="{color}">{name}</text>"""

svg += "\n</svg>\n"

out = ".github/assets/logo.svg"
with open(out, "w", encoding="utf-8") as f:
    f.write(svg)
print(f"Written {W}x{H} → {out}")
