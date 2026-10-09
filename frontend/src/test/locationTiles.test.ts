import { describe, it, expect } from "vitest";
import { TILE_URL, TILE_ATTRIBUTION } from "../components/widgets/locationTiles";

describe("LocationWidget tiles", () => {
  it("usa tiles de OpenStreetMap y no de CARTO", () => {
    expect(TILE_URL).toContain("openstreetmap.org");
    expect(TILE_URL).not.toContain("cartocdn");
  });

  it("atribuye a OpenStreetMap", () => {
    expect(TILE_ATTRIBUTION).toContain("OpenStreetMap");
  });
});
