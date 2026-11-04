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

  it("envía el valor de un campo numérico como número", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const data: QuickFormData = {
      title: "Presupuesto",
      fields: [{ name: "presupuesto", label: "Presupuesto", type: "number" }],
    };

    render(<QuickFormWidget data={data} onAction={onAction} />);

    await user.type(screen.getByLabelText("Presupuesto"), "1500");
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { presupuesto: 1500 });
  });

  it("renderiza un type `textarea` como área de texto y envía su valor", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const data: QuickFormData = {
      title: "Notas",
      fields: [{ name: "notas", label: "Notas", type: "textarea" }],
    };

    const { container } = render(
      <QuickFormWidget data={data} onAction={onAction} />,
    );

    expect(container.querySelector("textarea")).not.toBeNull();

    await user.type(screen.getByLabelText("Notas"), "Primera línea");
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", { notas: "Primera línea" });
  });

  it("usa `description` como título cuando falta `title`", () => {
    const data: QuickFormData = {
      description: "Elige una opción",
      fields: [{ name: "ciudad", label: "Ciudad", type: "text" }],
    };

    render(<QuickFormWidget data={data} onAction={vi.fn()} />);

    expect(screen.getByText("Elige una opción")).toBeInTheDocument();
  });

  it("muestra las alternativas de un campo `radio` a la vista y envía el texto elegido", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const data: QuickFormData = {
      fields: [
        {
          name: "choice",
          label: "Elige una opción",
          type: "radio",
          options: [
            "A — Cambiar el evento de hoy a «Rodaje suave»",
            "B — Dejarlo como «Andar suave 60 min»",
          ],
        },
      ],
      submit_label: "Enviar",
    };

    render(<QuickFormWidget data={data} onAction={onAction} />);

    // Las alternativas SHALL verse a la vista, sin desplegar ningún control.
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(
      screen.getByText("A — Cambiar el evento de hoy a «Rodaje suave»"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("B — Dejarlo como «Andar suave 60 min»"),
    ).toBeInTheDocument();
    // La etiqueta del campo SHALL actuar de pregunta.
    expect(screen.getByText("Elige una opción")).toBeInTheDocument();
    // El grupo SHALL exponerse como radiogroup y llevar la etiqueta del campo
    // como nombre accesible (la etiqueta apunta al grupo, no a un div mudo).
    expect(screen.getAllByRole("radiogroup")).toHaveLength(1);
    expect(
      screen.getByRole("radiogroup", { name: "Elige una opción" }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("radio", { name: "B — Dejarlo como «Andar suave 60 min»" }),
    );
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(onAction).toHaveBeenCalledWith("submit", {
      choice: "B — Dejarlo como «Andar suave 60 min»",
    });
  });
});