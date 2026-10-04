import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChecklistWidget } from "../components/widgets/ChecklistWidget";
import type { ChecklistData } from "../components/widgets/types";

beforeEach(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

const data: ChecklistData = {
  title: "Tareas del día",
  items: [
    { id: "a", label: "Comprar pan" },
    { id: "b", label: "Regar las plantas" },
    { id: "c", label: "Pasear al perro" },
  ],
};

describe("ChecklistWidget", () => {
  it("pinta el título y un checkbox por ítem", () => {
    render(<ChecklistWidget data={data} onAction={vi.fn()} />);

    expect(screen.getByText("Tareas del día")).toBeInTheDocument();
    expect(screen.getByLabelText("Comprar pan")).toBeInTheDocument();
    expect(screen.getByLabelText("Regar las plantas")).toBeInTheDocument();
    expect(screen.getByLabelText("Pasear al perro")).toBeInTheDocument();
  });

  it("envía los ids marcados", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();

    render(<ChecklistWidget data={data} onAction={onAction} />);

    await user.click(screen.getByLabelText("Comprar pan"));
    await user.click(screen.getByLabelText("Pasear al perro"));
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { checkedIds: ["a", "c"] });
  });

  it("envía una lista vacía si no hay nada marcado", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();

    render(<ChecklistWidget data={data} onAction={onAction} />);

    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { checkedIds: [] });
  });

  it("no revienta si `items` no es un array (degrada a lista vacía)", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const malformed = {
      title: "Malformado",
      items: "no-soy-un-array",
    } as unknown as ChecklistData;

    render(<ChecklistWidget data={malformed} onAction={onAction} />);

    expect(screen.getByText("Malformado")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { checkedIds: [] });
  });

  it("deshabilita el botón de envío cuando `disabled` es true", () => {
    render(<ChecklistWidget data={data} onAction={vi.fn()} disabled />);

    expect(screen.getByRole("button", { name: "Enviar" })).toBeDisabled();
  });
});