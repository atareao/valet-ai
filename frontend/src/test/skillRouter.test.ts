import { describe, it, expect } from "vitest";
import {
  changedSkillFields,
  isSkillFieldOverridden,
  skillEffectiveValues,
  skillEnabledKey,
  skillFieldKeys,
  skillFields,
  skillThresholdKey,
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
    enabled: true,
    question: "¿Pregunta efectiva?",
    criteria_true: "Criterio del sí efectivo",
    criteria_false: "Criterio del no efectivo",
    threshold: 0.1,
    overridden: [],
    ...overrides,
  };
}

describe("skillEnabledKey", () => {
  it("compone la clave de habilitación con el id en mayúsculas", () => {
    expect(skillEnabledKey("widgets")).toBe("ROUTER_SKILL_WIDGETS_ENABLED");
  });

  it("pasa el id a mayúsculas sin alterar el resto de la clave", () => {
    expect(skillEnabledKey("Pendientes")).toBe(
      "ROUTER_SKILL_PENDIENTES_ENABLED",
    );
  });
});

describe("skillThresholdKey", () => {
  it("compone la clave del umbral por skill con el id en mayúsculas", () => {
    expect(skillThresholdKey("widgets")).toBe("ROUTER_THRESHOLD_WIDGETS");
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
    expect(isSkillFieldOverridden(skill, "question", null)).toBe(true);
    expect(isSkillFieldOverridden(skill, "criteria_true", null)).toBe(true);
    expect(isSkillFieldOverridden(skill, "criteria_false", null)).toBe(false);
  });

  it("marca el fragmento cuando su clave en settings no está vacía", () => {
    const skill = makeSkill({ prompt_key: "SKILL_AGENDA_PROMPT" });
    const settings: Record<string, string> = {
      SKILL_AGENDA_PROMPT: "fragmento sobrescrito",
    };
    expect(isSkillFieldOverridden(skill, "prompt", settings)).toBe(true);
  });

  it("no marca el fragmento cuando su clave está vacía o ausente", () => {
    const skill = makeSkill({ prompt_key: "SKILL_AGENDA_PROMPT" });
    expect(isSkillFieldOverridden(skill, "prompt", null)).toBe(false);
    expect(
      isSkillFieldOverridden(skill, "prompt", { SKILL_AGENDA_PROMPT: "" }),
    ).toBe(false);
    expect(
      isSkillFieldOverridden(skill, "prompt", { SKILL_PENDIENTES_PROMPT: "x" }),
    ).toBe(false);
  });

  it("deja el fragmento fuera de `overridden` (solo settings lo marca)", () => {
    // `overridden` no cubre el fragmento: aunque no figure, settings manda.
    const overridden: SkillOverrideField[] = [
      "question",
      "criteria_true",
      "threshold",
    ];
    const skill = makeSkill({ overridden });
    expect(isSkillFieldOverridden(skill, "question", null)).toBe(true);
    // Con settings vacío, el fragmento no se marca pese a no estar en
    // `overridden`.
    expect(isSkillFieldOverridden(skill, "prompt", null)).toBe(false);
  });

  it("no marca nada con overridden vacío", () => {
    const skill = makeSkill({ overridden: [] });
    expect(isSkillFieldOverridden(skill, "question", null)).toBe(false);
    expect(isSkillFieldOverridden(skill, "criteria_true", null)).toBe(false);
    expect(isSkillFieldOverridden(skill, "criteria_false", null)).toBe(false);
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

  it("detecta el apagado de la habilitación y lo serializa", () => {
    const skill = makeSkill({ id: "widgets", enabled: true });
    const values: Record<string, unknown> = {
      ROUTER_SKILL_WIDGETS_ENABLED: false,
    };
    expect(changedSkillFields(values, [skill], null)).toEqual({
      ROUTER_SKILL_WIDGETS_ENABLED: "false",
    });
  });

  it("no envía la habilitación si no cambia", () => {
    const skill = makeSkill({ id: "widgets", enabled: true });
    const values: Record<string, unknown> = {
      ROUTER_SKILL_WIDGETS_ENABLED: true,
    };
    expect(changedSkillFields(values, [skill], null)).toEqual({});
  });

  it("trata la habilitación ausente en el catálogo como encendida", () => {
    const skill = makeSkill({ id: "widgets", enabled: undefined });
    // `true` coincide con el efectivo (por defecto habilitada) → sin cambio.
    expect(
      changedSkillFields({ ROUTER_SKILL_WIDGETS_ENABLED: true }, [skill], null),
    ).toEqual({});
    // `false` sí difiere → se envía serializado.
    expect(
      changedSkillFields(
        { ROUTER_SKILL_WIDGETS_ENABLED: false },
        [skill],
        null,
      ),
    ).toEqual({ ROUTER_SKILL_WIDGETS_ENABLED: "false" });
  });

  it("detecta el cambio de umbral por skill y lo serializa", () => {
    const skill = makeSkill({ id: "widgets", threshold: 0.2 });
    const values: Record<string, unknown> = {
      ROUTER_THRESHOLD_WIDGETS: 0.35,
    };
    expect(changedSkillFields(values, [skill], null)).toEqual({
      ROUTER_THRESHOLD_WIDGETS: "0.35",
    });
  });

  it("no envía el umbral si no cambia", () => {
    const skill = makeSkill({ id: "widgets", threshold: 0.2 });
    expect(
      changedSkillFields({ ROUTER_THRESHOLD_WIDGETS: 0.2 }, [skill], null),
    ).toEqual({});
  });

  it("combina los cuatro textos, la habilitación y el umbral en un solo diff", () => {
    const skill = makeSkill({
      id: "widgets",
      prompt_key: "SKILL_WIDGETS_PROMPT",
      question: "pregunta del catálogo",
      enabled: true,
      threshold: 0.2,
    });
    const values: Record<string, unknown> = {
      SKILL_WIDGETS_QUESTION: "pregunta editada",
      ROUTER_SKILL_WIDGETS_ENABLED: false,
      ROUTER_THRESHOLD_WIDGETS: 0.35,
    };
    expect(changedSkillFields(values, [skill], null)).toEqual({
      SKILL_WIDGETS_QUESTION: "pregunta editada",
      ROUTER_SKILL_WIDGETS_ENABLED: "false",
      ROUTER_THRESHOLD_WIDGETS: "0.35",
    });
  });

  it("no cambia nada con el catálogo vacío", () => {
    expect(changedSkillFields({ SKILL_X_PROMPT: "x" }, [], null)).toEqual({});
  });
});
