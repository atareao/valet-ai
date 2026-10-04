import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QuickFormWidget } from "../components/widgets/QuickFormWidget";
import type { QuickFormData, QuickFormField } from "../components/widgets/types";

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

describe("QuickFormWidget", () => {
  it("envía el valor de un campo de texto", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const data: QuickFormData = {
      title: "¿Dónde vas?",
      fields: [{ name: "ciudad", label: "Ciudad", type: "text" }],
    };

    render(<QuickFormWidget data={data} onAction={onAction} />);

    await user.type(screen.getByLabelText("Ciudad"), "Madrid");
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { ciudad: "Madrid" });
  });

  it("envía los valores de varios tipos de campo", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const data: QuickFormData = {
      title: "Planifica tu viaje",
      fields: [
        { name: "ciudad", label: "Ciudad", type: "text" },
        {
          name: "transporte",
          label: "Transporte",
          type: "select",
          options: ["tren", "avión"],
        },
        { name: "urgente", label: "Urgente", type: "checkbox" },
      ],
    };

    render(<QuickFormWidget data={data} onAction={onAction} />);

    await user.type(screen.getByLabelText("Ciudad"), "Madrid");
    await user.click(screen.getByRole("combobox"));
    // antd duplica el texto de la opción en varios nodos; `title` identifica el
    // elemento visible de la opción de forma única.
    await user.click(await screen.findByTitle("tren"));
    await user.click(screen.getByLabelText("Urgente"));
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", {
      ciudad: "Madrid",
      transporte: "tren",
      urgente: true,
    });
  });

  it("trata un type desconocido como campo de texto", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const desconocido = "fancy" as QuickFormField["type"];
    const data: QuickFormData = {
      title: "Raro",
      fields: [{ name: "nota", label: "Nota", type: desconocido }],
    };

    render(<QuickFormWidget data={data} onAction={onAction} />);

    await user.type(screen.getByLabelText("Nota"), "hola");
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { nota: "hola" });
  });

  it("usa submit_label cuando se proporciona", () => {
    const data: QuickFormData = {
      title: "X",
      fields: [],
      submit_label: "Confirmar",
    };

    render(<QuickFormWidget data={data} onAction={vi.fn()} />);

    expect(
      screen.getByRole("button", { name: "Confirmar" }),
    ).toBeInTheDocument();
  });

  it("no revienta si `fields` no es un array (degrada a lista vacía)", () => {
    const data = {
      title: "Malformado",
      fields: "no-soy-un-array",
    } as unknown as QuickFormData;

    render(<QuickFormWidget data={data} onAction={vi.fn()} />);

    expect(screen.getByText("Malformado")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Enviar" })).toBeInTheDocument();
  });

  it("no revienta si `options` no es un array (degrada a sin opciones)", () => {
    const data = {
      title: "Opciones raras",
      fields: [{ name: "x", label: "X", type: "select", options: 42 }],
    } as unknown as QuickFormData;

    render(<QuickFormWidget data={data} onAction={vi.fn()} />);

    expect(screen.getByText("Opciones raras")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toBeInTheDocument();
  });

  it("deshabilita el botón de envío cuando `disabled` es true", () => {
    const data: QuickFormData = {
      title: "X",
      fields: [{ name: "ciudad", label: "Ciudad", type: "text" }],
    };

    render(<QuickFormWidget data={data} onAction={vi.fn()} disabled />);

    expect(screen.getByRole("button", { name: "Enviar" })).toBeDisabled();
  });
});