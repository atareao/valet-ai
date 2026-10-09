import type { SkillInfo } from "../types";

// Módulo de lógica pura (sin componentes): aloja las constantes y funciones que
// comparten `RouterControl.tsx` y `SkillsTab.tsx`. Se separan del fichero del
// componente porque react-refresh solo admite ficheros que exporten
// exclusivamente componentes.

/** Etiqueta accesible del interruptor del enrutador. */
export const ROUTER_ENABLED_LABEL = "Activar enrutado";
/** Etiqueta accesible del campo del modelo de decisiones. */
export const ROUTER_MODEL_LABEL = "Modelo de decisiones";

/** En settings todo viaja como cadena: `"true"` es el único valor encendido. */
export function parseEnabled(value: string | undefined): boolean {
  return value === "true";
}

/**
 * Clave de settings del umbral por skill: `ROUTER_THRESHOLD_<ID>` con `<ID>` en
 * mayúsculas (p. ej. `ROUTER_THRESHOLD_WIDGETS`).
 */
export function skillThresholdKey(skillId: string): string {
  return `ROUTER_THRESHOLD_${skillId.toUpperCase()}`;
}

/**
 * Clave de settings de la habilitación por skill:
 * `ROUTER_SKILL_<ID>_ENABLED` con `<ID>` en mayúsculas (p. ej.
 * `ROUTER_SKILL_WIDGETS_ENABLED`). Por defecto, habilitada.
 */
export function skillEnabledKey(skillId: string): string {
  return `ROUTER_SKILL_${skillId.toUpperCase()}_ENABLED`;
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
 * Marca de un campo sobrescrito. La pregunta, los criterios y el umbral vienen
 * sobrescritos del catálogo (`overridden`); el fragmento se considera
 * sobrescrito cuando su clave en `settings` no está vacía (su default es la
 * ausencia, así que un valor no vacío equivale a un override).
 */
export function isSkillFieldOverridden(
  skill: SkillInfo,
  kind: SkillFieldKind,
  settings: Record<string, string> | null,
): boolean {
  if (kind === "prompt") {
    return (settings?.[skill.prompt_key] ?? "") !== "";
  }
  return skill.overridden.includes(kind);
}

/**
 * Diff de los campos de skill realmente modificados respecto al valor efectivo
 * vigente: los cuatro textos (pregunta, criterios y fragmento), la habilitación
 * (`ROUTER_SKILL_<ID>_ENABLED`, booleano en el formulario) y el umbral
 * (`ROUTER_THRESHOLD_<ID>`, numérico en el formulario). Se comparan solo los
 * campos presentes (`undefined` se ignora: un campo no registrado no puede
 * compararse) y se devuelven únicamente los que cambian, para no crear
 * sobrescrituras redundantes. La habilitación y el umbral se envían ya
 * serializados como cadena, que es como viaja todo en settings.
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

    const enabledKey = skillEnabledKey(skill.id);
    const enabledValue = values[enabledKey];
    if (
      typeof enabledValue === "boolean" &&
      enabledValue !== (skill.enabled ?? true)
    ) {
      changed[enabledKey] = enabledValue ? "true" : "false";
    }

    const thresholdKey = skillThresholdKey(skill.id);
    const thresholdValue = values[thresholdKey];
    if (
      typeof thresholdValue === "number" &&
      thresholdValue !== skill.threshold
    ) {
      changed[thresholdKey] = String(thresholdValue);
    }
  }
  return changed;
}
