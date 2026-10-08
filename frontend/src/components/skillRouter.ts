import type { SkillInfo } from "../types";

// Módulo de lógica pura (sin componentes): aloja las constantes y funciones que
// comparten `RouterControl.tsx` y `SkillPromptFields.tsx`. Se separan del
// fichero del componente porque react-refresh solo admite ficheros que
// exporten exclusivamente componentes.

/** Etiqueta accesible del interruptor del enrutador. */
export const ROUTER_ENABLED_LABEL = "Enrutador de skills";
/** Etiqueta accesible del campo del umbral. */
export const ROUTER_THRESHOLD_LABEL = "Umbral del enrutador";
/** Etiqueta accesible del campo del modelo de decisiones. */
export const ROUTER_MODEL_LABEL = "Modelo de decisiones";

/**
 * Valor por defecto del umbral si settings no lo trae o es ilegible. Refleja el
 * default del backend (`0.10`), al que cae `read_threshold` ante un valor
 * ausente o inválido.
 */
export const DEFAULT_THRESHOLD = 0.1;

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

/** Umbral con dos decimales, para mostrarlo sin ruido de coma flotante. */
export function formatThreshold(value: number): string {
  return value.toFixed(2);
}

/**
 * Clave de settings del umbral por skill: `ROUTER_THRESHOLD_<ID>` con `<ID>` en
 * mayúsculas (p. ej. `ROUTER_THRESHOLD_WIDGETS`).
 */
export function skillThresholdKey(skillId: string): string {
  return `ROUTER_THRESHOLD_${skillId.toUpperCase()}`;
}

/** Campo editable de una skill en la sub-pestaña «Skills». */
export type SkillFieldKind = "question" | "criteria_true" | "criteria_false" | "prompt";

export interface SkillFieldDescriptor {
  kind: SkillFieldKind;
  /** Clave de settings con la que viaja el campo en el formulario. */
  key: string;
  /** Etiqueta visible del campo. */
  label: string;
}

const SKILL_FIELD_LABELS: Record<SkillFieldKind, string> = {
  question: "Pregunta",
  criteria_true: "Criterio SÍ",
  criteria_false: "Criterio NO",
  prompt: "Fragmento de prompt",
};

const SKILL_FIELD_ORDER: SkillFieldKind[] = [
  "question",
  "criteria_true",
  "criteria_false",
  "prompt",
];

/**
 * Las cuatro claves de settings de una skill. La pregunta y los criterios usan
 * el id en mayúsculas; el fragmento es el `prompt_key` que ya devuelve el
 * catálogo.
 */
export function skillFieldKeys(skill: SkillInfo): Record<SkillFieldKind, string> {
  const id = skill.id.toUpperCase();
  return {
    question: `SKILL_${id}_QUESTION`,
    criteria_true: `SKILL_${id}_CRITERIA_TRUE`,
    criteria_false: `SKILL_${id}_CRITERIA_FALSE`,
    prompt: skill.prompt_key,
  };
}

/** Descriptores de los cuatro campos de una skill, en orden de presentación. */
export function skillFields(skill: SkillInfo): SkillFieldDescriptor[] {
  const keys = skillFieldKeys(skill);
  return SKILL_FIELD_ORDER.map((kind) => ({
    kind,
    key: keys[kind],
    label: SKILL_FIELD_LABELS[kind],
  }));
}

/**
 * Valor vigente **efectivo** de cada campo de una skill: pregunta, criterios y
 * umbral vienen del catálogo (ya resueltos); el fragmento vive en `settings`
 * (su default es la ausencia).
 */
export function skillEffectiveValues(
  skill: SkillInfo,
  settings: Record<string, string> | null,
): Record<string, string> {
  const keys = skillFieldKeys(skill);
  return {
    [keys.question]: skill.question,
    [keys.criteria_true]: skill.criteria_true,
    [keys.criteria_false]: skill.criteria_false,
    [keys.prompt]: settings?.[keys.prompt] ?? "",
  };
}

/**
 * Marca de un campo sobrescrito. Solo la pregunta, los criterios y el umbral
 * pueden venir sobrescritos (`overridden` del catálogo); el fragmento no se
 * marca.
 *
 * Precondición deliberada: `"prompt"` queda excluido, así que hoy es
 * inalcanzable restaurar un fragmento desde esta UI. Si algún día se marcara,
 * la acción de restaurar no refrescaría el valor mostrado: el fragmento se
 * recalcula de `settings`, que no se recarga tras el guardado (a diferencia de
 * la pregunta y los criterios, que vienen del catálogo releído). Habría que
 * releer también `settings` antes de habilitarla.
 */
export function isSkillFieldOverridden(
  skill: SkillInfo,
  kind: SkillFieldKind,
): boolean {
  return kind !== "prompt" && skill.overridden.includes(kind);
}

/**
 * Diff de los campos de skill realmente modificados respecto al valor efectivo
 * vigente. Se comparan solo los campos presentes en el formulario (`undefined`
 * se ignora: un campo no registrado no puede compararse) y se devuelven
 * únicamente los que cambian, para no crear sobrescrituras redundantes.
 */
export function changedSkillFields(
  values: Record<string, unknown>,
  skills: SkillInfo[],
  settings: Record<string, string> | null,
): Record<string, string> {
  const changed: Record<string, string> = {};
  for (const skill of skills) {
    const effective = skillEffectiveValues(skill, settings);
    for (const [key, value] of Object.entries(effective)) {
      const current = values[key];
      if (typeof current === "string" && current !== value) {
        changed[key] = current;
      }
    }
  }
  return changed;
}
