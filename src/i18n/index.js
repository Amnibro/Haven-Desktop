import en from './locales/en.js';
import ptBR from './locales/pt-BR.js';

export const DEFAULT_LOCALE = 'en';
export const AUTOMATIC_LANGUAGE = 'auto';
export const LEGACY_SYSTEM_LANGUAGE = 'system';
export const SYSTEM_LANGUAGE = AUTOMATIC_LANGUAGE;

export const LOCALES = Object.freeze({
  en,
  'pt-BR': ptBR,
});

export const SUPPORTED_LOCALES = Object.freeze([
  Object.freeze({ code: 'en', name: 'English', direction: 'ltr' }),
  Object.freeze({ code: 'pt-BR', name: 'Português (Brasil)', direction: 'ltr' }),
]);

export function normalizeLocale(value) {
  const candidate = String(value || '').trim().replace(/_/g, '-');
  if (!candidate) return null;

  const exact = SUPPORTED_LOCALES.find(({ code }) => code.toLowerCase() === candidate.toLowerCase());
  if (exact) return exact.code;

  const base = candidate.split('-')[0].toLowerCase();
  const baseMatch = SUPPORTED_LOCALES.find(({ code }) => code.split('-')[0].toLowerCase() === base);
  return baseMatch?.code || null;
}

export function resolveLocale(preference, systemLanguages = []) {
  if (preference && preference !== AUTOMATIC_LANGUAGE && preference !== LEGACY_SYSTEM_LANGUAGE) {
    return normalizeLocale(preference) || DEFAULT_LOCALE;
  }

  for (const language of systemLanguages) {
    const locale = normalizeLocale(language);
    if (locale) return locale;
  }
  return DEFAULT_LOCALE;
}

function interpolate(message, values = {}) {
  return String(message).replace(/\{([A-Za-z0-9_]+)\}/g, (match, name) => {
    return Object.prototype.hasOwnProperty.call(values, name) ? String(values[name]) : match;
  });
}

export function translate(locale, key, values) {
  const resolved = normalizeLocale(locale) || DEFAULT_LOCALE;
  const message = LOCALES[resolved]?.[key] ?? LOCALES[DEFAULT_LOCALE]?.[key];
  return interpolate(message ?? key, values);
}

export function createTranslator(locale) {
  return (key, values) => translate(locale, key, values);
}

export function getLocaleMetadata(locale) {
  const resolved = normalizeLocale(locale) || DEFAULT_LOCALE;
  return SUPPORTED_LOCALES.find(({ code }) => code === resolved);
}
