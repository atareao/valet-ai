import { describe, it, expect } from "vitest";
import {
  changedSkillFields,
  DEFAULT_THRESHOLD,
  formatThreshold,
  isSkillFieldOverridden,
  parseThreshold,
  skillEffectiveValues,
  skillFieldKeys,
  skillFields,
} from "../components/skillRouter";
import type { SkillInfo, SkillOverrideField } from "../types";

// Skill de catálogo con todos sus campos; los tests sobrescriben solo lo que
// les interesa para que cada caso se lea de un vistazo.
function makeSkill(overrides: Partial<SkillInfo> = {}): SkillInfo {
  return {
    id: "pendientes",
    prompt_key: "SKILL_PENDIENTES_PROMPT",
    prompt_heading: "# SKILL ACTIVA: PENDIENTES",
    tools: ["tasks"],
    question: "¿Pregunta efectiva?",
    criteria_true: "Criterio del sí efectivo",
    criteria_false: "Criterio del no efectivo",
    threshold: 0.1,
    overridden: [],
    ...overrides,
  };
}

describe("parseThreshold", () => {
  it("conserva un valor válido dentro de [0, 1]", () => {
    expect(parseThreshold("0.45")).toBe(0.45);
  });

  it("cae al default cuando el valor está ausente", () => {
    expect(parseThreshold(undefined)).toBe(DEFAULT_THRESHOLD);
  });

  it("cae al default para una cadena vacía", () => {
    expect(parseThreshold("")).toBe(DEFAULT_THRESHOLD);
  });

  it("cae al default para un valor no numérico", () => {
    expect(parseThreshold("no-numero")).toBe(DEFAULT_THRESHOLD);
  });

  it("cae al default para NaN literal", () => {
    expect(parseThreshold("NaN")).toBe(DEFAULT_THRESHOLD);
  });

  it("cae al default para infinito", () => {
    expect(parseThreshold("inf")).toBe(DEFAULT_THRESHOLD);
  });

  it("cae al default para un valor fuera de rango por arriba", () => {
    expect(parseThreshold("2.5")).toBe(DEFAULT_THRESHOLD);
  });

  it("cae al default para un valor fuera de rango por abajo", () => {
    expect(parseThreshold("-1")).toBe(DEFAULT_THRESHOLD);
  });

  it("conserva el extremo inferior válido 0", () => {
    expect(parseThreshold("0")).toBe(0);
  });

  it("conserva el extremo superior válido 1", () => {
    expect(parseThreshold("1")).toBe(1);
  });
});

describe("formatThreshold", () => {
  it("formatea con dos decimales", () => {
    expect(formatThreshold(0)).toBe("0.00");
    expect(formatThreshold(0.1)).toBe("0.10");
    expect(formatThreshold(0.2)).toBe("0.20");
    expect(formatThreshold(0.333)).toBe("0.33");
    expect(formatThreshold(1)).toBe("1.00");
  });
});

describe("skillFieldKeys", () => {
  it("compone las cuatro claves de una skill con el id en mayúsculas", () => {
    expect(skillFieldKeys(makeSkill({ id: "pendientes" }))).toEqual({
      question: "SKILL_PENDIENTES_QUESTION",
      criteria_true: "SKILL_PENDIENTES_CRITERIA_TRUE",
      criteria_false: "SKILL_PENDIENTES_CRITERIA_FALSE",
      prompt: "SKILL_PENDIENTES_PROMPT",
    });
  });

  it("usa el prompt_key del catálogo para el fragmento", () => {
    expect(
      skillFieldKeys(makeSkill({ prompt_key: "SKILL_WIDGETS_PROMPT" })).prompt,
    ).toBe("SKILL_WIDGETS_PROMPT");
  });

  it("pasa el id a mayúsculas sin alterar el resto de la clave", () => {
    expect(skillFieldKeys(makeSkill({ id: "agenda" })).question).toBe(
      "SKILL_AGENDA_QUESTION",
    );
  });
});

describe("skillFields", () => {
  it("devuelve los cuatro descriptores con sus claves correctas", () => {
    const skill = makeSkill({ id: "agenda", prompt_key: "SKILL_AGENDA_PROMPT" });
    expect(skillFields(skill)).toEqual([
      { kind: "question", key: "SKILL_AGENDA_QUESTION", label: "Pregunta" },
      {
        kind: "criteria_true",
        key: "SKILL_AGENDA_CRITERIA_TRUE",
        label: "Criterio SÍ",
      },
      {
        kind: "criteria_false",
        key: "SKILL_AGENDA_CRITERIA_FALSE",
        label: "Criterio NO",
      },
      {
        kind: "prompt",
        key: "SKILL_AGENDA_PROMPT",
        label: "Fragmento de prompt",
      },
    ]);
  });

  it("usa el prompt_key del catálogo en el descriptor del fragmento", () => {
    const fields = skillFields(
      makeSkill({ id: "widgets", prompt_key: "SKILL_WIDGETS_PROMPT" }),
    );
    expect(fields.find((field) => field.kind === "prompt")?.key).toBe(
      "SKILL_WIDGETS_PROMPT",
    );
  });
});

describe("skillEffectiveValues", () => {
  it("toma la pregunta y los criterios del catálogo y el fragmento de settings", () => {
    const skill = makeSkill({
      id: "agenda",
      prompt_key: "SKILL_AGENDA_PROMPT",
      question: "pregunta del catálogo",
      criteria_true: "criterio sí del catálogo",
      criteria_false: "criterio no del catálogo",
    });
    const settings: Record<string, string> = {
      SKILL_AGENDA_PROMPT: "fragmento vigente",
    };
    expect(skillEffectiveValues(skill, settings)).toEqual({
      SKILL_AGENDA_QUESTION: "pregunta del catálogo",
      SKILL_AGENDA_CRITERIA_TRUE: "criterio sí del catálogo",
      SKILL_AGENDA_CRITERIA_FALSE: "criterio no del catálogo",
      SKILL_AGENDA_PROMPT: "fragmento vigente",
    });
  });

  it("deja el fragmento vacío si settings no lo trae", () => {
    const skill = makeSkill({ id: "agenda", prompt_key: "SKILL_AGENDA_PROMPT" });
    expect(skillEffectiveValues(skill, null)).toEqual({
      SKILL_AGENDA_QUESTION: skill.question,
      SKILL_AGENDA_CRITERIA_TRUE: skill.criteria_true,
      SKILL_AGENDA_CRITERIA_FALSE: skill.criteria_false,
      SKILL_AGENDA_PROMPT: "",
    });
  });
});

describe("isSkillFieldOverridden", () => {
  it("marca el campo cuyo kind figura en overridden", () => {
    const skill = makeSkill({ overridden: ["question", "criteria_true"] });
    expect(isSkillFieldOverridden(skill, "question")).toBe(true);
    expect(isSkillFieldOverridden(skill, "criteria_true")).toBe(true);
    expect(isSkillFieldOverridden(skill, "criteria_false")).toBe(false);
  });

  it("nunca marca el fragmento aunque haya otros campos sobrescritos", () => {
    // `overridden` solo cubre pregunta, criterios y umbral: el fragmento no se
    // marca (su restauración es vaciar su clave). El tipo `SkillOverrideField`
    // ni siquiera admite `"prompt"`, así que basta con partir de campos válidos
    // y comprobar que el kind del fragmento nunca resulta marcado.
    const overridden: SkillOverrideField[] = [
      "question",
      "criteria_true",
      "threshold",
    ];
    const skill = makeSkill({ overridden });
    expect(isSkillFieldOverridden(skill, "question")).toBe(true);
    expect(isSkillFieldOverridden(skill, "prompt")).toBe(false);
  });

  it("no marca nada con overridden vacío", () => {
    const skill = makeSkill({ overridden: [] });
    expect(isSkillFieldOverridden(skill, "question")).toBe(false);
    expect(isSkillFieldOverridden(skill, "criteria_true")).toBe(false);
    expect(isSkillFieldOverridden(skill, "criteria_false")).toBe(false);
  });
});

describe("changedSkillFields", () => {
  it("devuelve solo los campos que difieren del valor efectivo", () => {
    const skill = makeSkill({ id: "agenda", prompt_key: "SKILL_AGENDA_PROMPT" });
    const settings: Record<string, string> = {
      SKILL_AGENDA_PROMPT: "fragmento vigente",
    };
    const values: Record<string, unknown> = {
      // Iguales al efectivo → fuera del diff.
      SKILL_AGENDA_QUESTION: skill.question,
      SKILL_AGENDA_CRITERIA_TRUE: skill.criteria_true,
      SKILL_AGENDA_CRITERIA_FALSE: skill.criteria_false,
      // Distinto del fragmento vigente → dentro.
      SKILL_AGENDA_PROMPT: "fragmento editado",
    };
    expect(changedSkillFields(values, [skill], settings)).toEqual({
      SKILL_AGENDA_PROMPT: "fragmento editado",
    });
  });

  it("ignora los campos no registrados (undefined) y las claves ajenas", () => {
    const skill = makeSkill({ id: "agenda", prompt_key: "SKILL_AGENDA_PROMPT" });
    const values: Record<string, unknown> = {
      system_prompt: "otro texto",
      SKILL_AGENDA_QUESTION: undefined,
    };
    expect(changedSkillFields(values, [skill], null)).toEqual({});
  });

  it("registra el vaciado explícito de un campo como cambio", () => {
    const skill = makeSkill({ id: "agenda", prompt_key: "SKILL_AGENDA_PROMPT" });
    const values: Record<string, unknown> = {
      SKILL_AGENDA_CRITERIA_TRUE: "",
    };
    expect(changedSkillFields(values, [skill], null)).toEqual({
      SKILL_AGENDA_CRITERIA_TRUE: "",
    });
  });

  it("compara la pregunta y los criterios contra el catálogo y el fragmento contra settings", () => {
    const skill = makeSkill({
      id: "agenda",
      prompt_key: "SKILL_AGENDA_PROMPT",
      question: "pregunta del catálogo",
    });
    const settings: Record<string, string> = {
      SKILL_AGENDA_PROMPT: "fragmento del catálogo",
    };
    const values: Record<string, unknown> = {
      SKILL_AGENDA_QUESTION: "pregunta editada",
      SKILL_AGENDA_PROMPT: "fragmento del catálogo",
    };
    expect(changedSkillFields(values, [skill], settings)).toEqual({
      SKILL_AGENDA_QUESTION: "pregunta editada",
    });
  });

  it("no cambia nada con el catálogo vacío", () => {
    expect(changedSkillFields({ SKILL_X_PROMPT: "x" }, [], null)).toEqual({});
  });
});
