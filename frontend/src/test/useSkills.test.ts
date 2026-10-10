import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el cliente API (no `fetch`) siguiendo el patrón de
// `usePersistentMemory.test.ts`. Se conserva el resto del cliente con
// `importActual` para no acoplar el test a los demás endpoints.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      getSkills: vi.fn(),
    },
  };
});

import { api } from "../api/client";
import { useSkills } from "../hooks/useSkills";
import type { SkillsResponse } from "../types";

const mockGetSkills = vi.mocked(api.getSkills);

const skillsFixture: SkillsResponse = {
  skills: [
    {
      id: "agenda",
      prompt_key: "SKILL_AGENDA_PROMPT",
      prompt_heading: "# SKILL ACTIVA: AGENDA",
      tools: ["calendar"],
      enabled: true,
      question: "¿La respuesta requiere mirar la agenda?",
      criteria_true: "El mensaje se refiere a eventos.",
      criteria_false: "El mensaje no se refiere a nada programado.",
      threshold: 0.1,
      overridden: [],
    },
    {
      id: "widgets",
      prompt_key: "SKILL_WIDGETS_PROMPT",
      prompt_heading: "# SKILL ACTIVA: WIDGETS",
      tools: ["render_widget"],
      enabled: false,
      question: "¿La respuesta requiere mostrar algo interactivo?",
      criteria_true: "El turno implica pedir varios datos a la vez.",
      criteria_false: "El turno se resuelve con una explicación.",
      threshold: 0.2,
      overridden: ["threshold"],
    },
  ],
  core_tools: ["render_widget", "get_current_time"],
};

describe("useSkills", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetSkills.mockResolvedValue(skillsFixture);
  });

  it("carga el catálogo al montar y expone skills/loading", async () => {
    const { result } = renderHook(() => useSkills());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(mockGetSkills).toHaveBeenCalledTimes(1);
    expect(result.current.skills).toEqual(skillsFixture.skills);
    expect(result.current.error).toBeNull();
  });

  it("expone los skills con su estado enabled", async () => {
    const { result } = renderHook(() => useSkills());
    await waitFor(() => expect(result.current.loading).toBe(false));

    const widgets = result.current.skills.find(
      (skill) => skill.id === "widgets",
    );
    expect(widgets?.enabled).toBe(false);
  });

  it("si la consulta falla fija error sin re-lanzar desde el hook", async () => {
    mockGetSkills.mockRejectedValue(new Error("boom"));
    const { result } = renderHook(() => useSkills());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBeTruthy();
    expect(result.current.skills).toEqual([]);
  });

  it("refetch relee el catálogo y devuelve la respuesta nueva", async () => {
    const { result } = renderHook(() => useSkills());
    await waitFor(() => expect(result.current.loading).toBe(false));

    const updated: SkillsResponse = {
      ...skillsFixture,
      skills: skillsFixture.skills.map((skill) => ({
        ...skill,
        enabled: true,
      })),
    };
    mockGetSkills.mockResolvedValueOnce(updated);

    let returned: SkillsResponse | undefined;
    await act(async () => {
      returned = await result.current.refetch();
    });

    expect(returned).toEqual(updated);
    expect(result.current.skills.every((skill) => skill.enabled)).toBe(true);
    expect(mockGetSkills).toHaveBeenCalledTimes(2);
  });

  it("refetch rechaza si la llamada falla y conserva el estado previo", async () => {
    const { result } = renderHook(() => useSkills());
    await waitFor(() => expect(result.current.loading).toBe(false));

    mockGetSkills.mockRejectedValueOnce(new Error("boom"));

    await act(async () => {
      await expect(result.current.refetch()).rejects.toThrow("boom");
    });

    expect(result.current.skills).toEqual(skillsFixture.skills);
  });
});
