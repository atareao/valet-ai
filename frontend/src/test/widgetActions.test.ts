import { describe, it, expect } from "vitest";
import { formatWidgetAction } from "../components/widgets/actions";

describe("formatWidgetAction", () => {
  it("produce la línea exacta para un submit de QuickForm", () => {
    expect(
      formatWidgetAction("QuickForm", "abc", "submit", { ciudad: "Madrid" }),
    ).toBe('[widget:QuickForm#abc] submit {"ciudad":"Madrid"}');
  });

  it("produce la línea exacta para un submit de Checklist", () => {
    expect(
      formatWidgetAction("Checklist", "def", "submit", {
        checkedIds: ["a", "c"],
      }),
    ).toBe('[widget:Checklist#def] submit {"checkedIds":["a","c"]}');
  });
});