'use strict';

/**
 * Amniscient dark-engineer tokens for Amni Relay chrome and overlay.
 * Not a Haven redesign — a product skin applied by Relay, not a flag on Haven.
 */
const tokens = {
  bg: '#0b0d10',
  surface: '#12161c',
  surface2: '#1a2028',
  border: '#2a333e',
  accent: '#3ecfaf',
  accentHover: '#4ee0c0',
  accentDim: '#2a9d86',
  accentGlow: 'rgba(62, 207, 175, 0.32)',
  brass: '#e8b86d',
  text: '#e8edf2',
  textDim: '#8b96a3',
  textMuted: '#5c6670',
  success: '#3ecfaf',
  danger: '#e26d6d',
  radius: '8px',
  font:
    '"IBM Plex Sans", "Segoe UI", system-ui, -apple-system, sans-serif',
  mono: '"IBM Plex Mono", "Cascadia Code", "Fira Code", monospace',
};

function cssVariables(prefix = 'amni') {
  return {
    [`--${prefix}-bg`]: tokens.bg,
    [`--${prefix}-surface`]: tokens.surface,
    [`--${prefix}-surface-2`]: tokens.surface2,
    [`--${prefix}-border`]: tokens.border,
    [`--${prefix}-accent`]: tokens.accent,
    [`--${prefix}-accent-hover`]: tokens.accentHover,
    [`--${prefix}-accent-dim`]: tokens.accentDim,
    [`--${prefix}-accent-glow`]: tokens.accentGlow,
    [`--${prefix}-brass`]: tokens.brass,
    [`--${prefix}-text`]: tokens.text,
    [`--${prefix}-text-dim`]: tokens.textDim,
    [`--${prefix}-text-muted`]: tokens.textMuted,
    [`--${prefix}-success`]: tokens.success,
    [`--${prefix}-danger`]: tokens.danger,
    [`--${prefix}-radius`]: tokens.radius,
    [`--${prefix}-font`]: tokens.font,
    [`--${prefix}-mono`]: tokens.mono,
  };
}

function cssVariableBlock(selector = ':root') {
  const vars = cssVariables();
  const body = Object.entries(vars)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join('\n');
  return `${selector} {\n${body}\n}`;
}

module.exports = {
  tokens,
  cssVariables,
  cssVariableBlock,
};
