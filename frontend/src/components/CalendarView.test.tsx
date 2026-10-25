import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import type { ReactNode } from "react";
import dayjs from "dayjs";
import type { CalendarEvent } from "../types";

// Se mockea solo el cliente HTTP: el hook `useEvents` y la vista corren de
// verdad (spec `frontend/agenda`). `CalendarView` necesita `matchMedia` (antd)
// y el cliente real pegaría contra la red, así que el mock es un objeto propio
// (hoisted), como en el RED de `NotesView`/`JournalView`.
const mockApi = vi.hoisted(() => ({
  listEvents: vi.fn(),
  createEvent: vi.fn(),
  updateEvent: vi.fn(),
  deleteEvent: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: mockApi,
  BASE_URL: "/api",
}));

import { CalendarView } from "./CalendarView";

beforeEach(() => {
  vi.clearAllMocks();
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

/**
 * Un evento de hoy, para que caiga siempre en el mes que el calendario pinta.
 * La hora se compone en local (`format`, no `toISOString`) para no desplazar el
 * día al oeste de UTC.
 */
const event = (overrides: Partial<CalendarEvent> = {}): CalendarEvent => ({
  id: "ev1",
  profile_id: "profile-id",
  title: "Reunión",
  start_time: dayjs().hour(10).minute(0).second(0).format("YYYY-MM-DDTHH:mm:ss"),
  end_time: dayjs().hour(11).minute(0).second(0).format("YYYY-MM-DDTHH:mm:ss"),
  scope: "personal",
  category: "work",
  all_day: false,
  created_at: "2026-10-18T09:00:00",
  updated_at: "2026-10-18T09:00:00",
  ...overrides,
});

// `CalendarView` usa `App.useApp()` para el aviso de error (`message.error`),
// así que el render necesita el contexto de `<AntdApp>` (patrón del repo).
const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

/** Abre el detalle de un evento clicando su insignia en el calendario. */
const openDetail = async (
  user: ReturnType<typeof userEvent.setup>,
  title: string,
) => {
  await user.click(await screen.findByText(title));
  return await screen.findByRole("dialog");
};

/** Confirma el `Popconfirm` del botón «Delete» del detalle. */
const confirmDelete = async (user: ReturnType<typeof userEvent.setup>) => {
  const dialog = screen.getByRole("dialog");
  // El icono de antd suma su `aria-label` al nombre accesible del botón
  // («delete Delete»), así que se compara sin anclas.
  await user.click(within(dialog).getByRole("button", { name: /Delete/ }));

  // El popconfirm abre su burbuja con otro botón «Delete» (el `okText`).
  // El del portal se añade después del trigger en el DOM.
  expect(
    await screen.findByText("Are you sure you want to delete this event?"),
  ).toBeInTheDocument();
  const deleteButtons = screen.getAllByRole("button", { name: /Delete/ });
  await user.click(deleteButtons[deleteButtons.length - 1]);
};

describe("CalendarView — borrado de eventos", () => {
  it("borra de verdad: llama a deleteEvent con el id y recarga el listado", async () => {
    const user = userEvent.setup();
    mockApi.listEvents.mockResolvedValue([event()]);
    mockApi.deleteEvent.mockResolvedValue(undefined);

    render(<CalendarView />, { wrapper: AppWrapper });

    await openDetail(user, "Reunión");
    expect(mockApi.listEvents).toHaveBeenCalledTimes(1);

    await confirmDelete(user);

    await waitFor(() => {
      expect(mockApi.deleteEvent).toHaveBeenCalledWith("ev1");
    });
    // Tras un borrado con éxito, el listado se recarga.
    await waitFor(() => {
      expect(mockApi.listEvents).toHaveBeenCalledTimes(2);
    });
    // Y el detalle se cierra (spec `frontend/agenda`, «Delete event from
    // calendar»). Ojo: en jsdom el nodo del `Modal` de antd NO desaparece del
    // DOM. `rc-motion` aguarda un `transitionend`/`animationend` que jsdom nunca
    // emite y `Modal` no pasa `motionDeadline`, así que la animación de salida
    // no «termina» y el nodo queda montado (misma limitación documentada en
    // `SettingsDialog.test.tsx`). La señal equivalente es la de `rc-motion`: al
    // procesar antd `open=false`, el nodo recibe `ant-zoom-leave`. Si el borrado
    // con éxito no cerrase el detalle, seguiría en `ant-zoom-enter-active` y
    // esta aserción fallaría.
    await waitFor(() => {
      const dialog = screen.queryByRole("dialog");
      if (dialog) expect(dialog).toHaveClass("ant-zoom-leave");
    });
  });

  it("un borrado fallido avisa y deja el detalle abierto", async () => {
    const user = userEvent.setup();
    mockApi.listEvents.mockResolvedValue([event()]);
    mockApi.deleteEvent.mockRejectedValue(new Error("boom"));

    render(<CalendarView />, { wrapper: AppWrapper });

    await openDetail(user, "Reunión");

    await confirmDelete(user);

    // Aviso de error visible (patrón `message.error` del repo: el mensaje).
    expect(await screen.findByText("boom")).toBeInTheDocument();
    // El detalle sigue abierto con el evento (no se finge el borrado).
    expect(
      within(screen.getByRole("dialog")).getByText("Reunión"),
    ).toBeInTheDocument();
  });
});
