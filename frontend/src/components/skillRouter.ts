import type { SkillInfo } from "../types";

// Módulo de lógica puro (sin componentes): aloja las constantes y funciones que
// compartían `RouterControl.tsx` y `SkillPromptFields.tsx`. Se separan del
// fichero del componente porque react-refresh solo admite ficheros que
// exporten exclusivamente componentes.

/** Etiqueta accesible del interruptor del enrutador. */
export const ROUTER_ENABLED_LABEL = "Enrutador de skills";
/** Etiqueta accesible del campo del umbral. */
export const ROUTER_THRESHOLD_LABEL = "Umbral del enrutador";
/** Etiqueta accesible del campo del modelo de decisiones. */
export const ROUTER_MODEL_LABEL = "Modelo de decisiones";

/** Valor por defecto del umbral si settings no lo trae o es ilegible. */
export const DEFAULT_THRESHOLD = 0.3;

/** En settings todo viaja como cadena: `"true"` es el único valor encendido. */
export function parseEnabled(value: string | undefined): boolean {
  return value === "true";
}

/**
 * Umbral como número. Refleja la validación del backend (`read_threshold` en
 * `skill_router.rs`): un valor solo es válido si es finito y está en `[0, 1]`;
 * cualquier otro caso (ausente, ilegible, `NaN`, infinito o fuera de rango)
 * cae al default, igual que haría el backend. Los extremos `"0"` y `"1"` son
 * válidos y se conservan.
 */
export function parseThreshold(value: string | undefined): number {
  const parsed = parseFloat(value ?? "");
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 1) {
    return DEFAULT_THRESHOLD;
  }
  return parsed;
}

/** Clave de settings de un fragmento de skill: `SKILL_<ID>_PROMPT`. */
const SKILL_PROMPT_KEY_RE = /^SKILL_.+_PROMPT$/;

/**
 * Claves de fragmento presentes en el objeto de settings, sin enumerarlas a
 * mano: si mañana se siembra una skill nueva, aparece sola en el formulario.
 */
export function collectSkillPromptKeys(
  settings: Record<string, string> | null,
): string[] {
  if (!settings) return [];
  return Object.keys(settings).filter((key) => SKILL_PROMPT_KEY_RE.test(key));
}

export interface SkillField {
  key: string;
  label: string;
}

/**
 * Ordena las claves según el catálogo (`GET /api/skills`) y las etiqueta con el
 * id de la skill. Las claves ausentes del catálogo se colocan al final con la
 * propia clave como etiqueta, de modo que un fallo de la API nunca oculta un
 * fragmento ya configurado.
 */
export function orderSkillFields(
  keys: string[],
  skills: SkillInfo[],
): SkillField[] {
  const byKey = new Map(skills.map((skill) => [skill.prompt_key, skill]));
  const catalogKeys = skills.map((skill) => skill.prompt_key);
  const ordered = [
    ...catalogKeys.filter((key) => keys.includes(key)),
    ...keys.filter((key) => !catalogKeys.includes(key)),
  ];
  return ordered.map((key) => ({ key, label: byKey.get(key)?.id ?? key }));
}
