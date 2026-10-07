import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { AppLoader } from "../components/AppLoader";

describe("AppLoader", () => {
  it("muestra el indicador de carga sin depender de antd", () => {
    const { container } = render(<AppLoader />);

    expect(screen.getByTestId("auth-loading")).toBeInTheDocument();
    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(container.querySelector(".ant-spin")).toBeNull();
  });
});
