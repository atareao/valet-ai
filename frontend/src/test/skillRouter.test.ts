import { describe, it, expect } from "vitest";
import {
  collectSkillPromptKeys,
  DEFAULT_THRESHOLD,
  orderSkillFields,
  parseThreshold,
} from "../components/skillRouter";
import type { SkillInfo } from "../types";

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

describe("orderSkillFields", () => {
  const skills: SkillInfo[] = [
    { id: "beta", prompt_key: "SKILL_BETA_PROMPT" },
    { id: "alfa", prompt_key: "SKILL_ALFA_PROMPT" },
  ] as SkillInfo[];

  it("ordena las claves conocidas según el catálogo y etiqueta con el id", () => {
    const fields = orderSkillFields(
      ["SKILL_ALFA_PROMPT", "SKILL_BETA_PROMPT"],
      skills,
    );
    expect(fields).toEqual([
      { key: "SKILL_BETA_PROMPT", label: "beta" },
      { key: "SKILL_ALFA_PROMPT", label: "alfa" },
    ]);
  });

  it("coloca las claves desconocidas al final sin ocultarlas", () => {
    const fields = orderSkillFields(
      ["SKILL_DESCONOCIDA_PROMPT", "SKILL_ALFA_PROMPT"],
      skills,
    );
    expect(fields).toEqual([
      { key: "SKILL_ALFA_PROMPT", label: "alfa" },
      { key: "SKILL_DESCONOCIDA_PROMPT", label: "SKILL_DESCONOCIDA_PROMPT" },
    ]);
  });

  it("conserva todas las claves con un catálogo vacío", () => {
    const keys = ["SKILL_ALFA_PROMPT", "SKILL_X_PROMPT"];
    const fields = orderSkillFields(keys, []);
    expect(fields.map((field) => field.key)).toEqual(keys);
    expect(fields.every((field) => field.label === field.key)).toBe(true);
  });

  it("conserva todas las claves cuando el catálogo viene vacío por fallo", () => {
    const keys = ["SKILL_ALFA_PROMPT", "SKILL_BETA_PROMPT"];
    const fields = orderSkillFields(keys, []);
    expect(fields).toHaveLength(keys.length);
  });
});

describe("collectSkillPromptKeys", () => {
  it("filtra solo las claves SKILL_<ID>_PROMPT", () => {
    const settings: Record<string, string> = {
      SKILL_ALFA_PROMPT: "texto",
      SKILL_BETA_PROMPT: "otro",
      SKILL__PROMPT: "sin id",
      SKILL_X_PROMPT_EXTRA: "sufijo",
      ROUTER_ENABLED: "true",
      OTRA_CLAVE: "valor",
    };
    expect(collectSkillPromptKeys(settings)).toEqual([
      "SKILL_ALFA_PROMPT",
      "SKILL_BETA_PROMPT",
    ]);
  });

  it("no rompe con valores vacíos o ausentes", () => {
    const settings: Record<string, string> = {
      SKILL_ALFA_PROMPT: "",
    };
    const partial = { SKILL_BETA_PROMPT: undefined } as unknown as Record<
      string,
      string
    >;
    expect(collectSkillPromptKeys(settings)).toEqual(["SKILL_ALFA_PROMPT"]);
    expect(collectSkillPromptKeys(partial)).toEqual(["SKILL_BETA_PROMPT"]);
  });

  it("devuelve vacío para settings nulo", () => {
    expect(collectSkillPromptKeys(null)).toEqual([]);
  });
});
