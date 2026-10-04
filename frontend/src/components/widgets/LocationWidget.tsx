import type { FC } from "react";
import { Alert, App, Button, Typography } from "antd";
import {
  MapContainer,
  TileLayer,
  CircleMarker,
  Popup,
} from "react-leaflet";
import "leaflet/dist/leaflet.css";
import type { LocationData, WidgetProps } from "./types";

const { Text } = Typography;

const TILE_URL =
  "https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png";
const TILE_ATTRIBUTION =
  '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> &copy; <a href="https://carto.com/attributions">CARTO</a>';

/** Estrecha a `number` solo si es un número finito (descarta NaN/±Infinity). */
const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

/**
 * Mapa de una ubicación con sus acciones. Tolera datos incompletos: si las
 * coordenadas no son números finitos, muestra un aviso en lugar del mapa y no
 * lanza. Emite `onAction("save_place", { title, address, latitude, longitude })`
 * al pulsar «Guardar».
 */
export const LocationWidget: FC<WidgetProps<LocationData>> = ({
  data,
  onAction,
  disabled,
}) => {
  const { message } = App.useApp();
  const { latitude, longitude, title, address, description } = data ?? {};

  if (!isFiniteNumber(latitude) || !isFiniteNumber(longitude)) {
    return (
      <Alert
        type="warning"
        showIcon
        message="Ubicación sin coordenadas válidas"
        description="No es posible mostrar el mapa."
      />
    );
  }

  const lat = latitude;
  const lon = longitude;

  const handleSave = () => {
    onAction("save_place", { title, address, latitude: lat, longitude: lon });
  };

  const handleDirections = () => {
    const opened = window.open(
      `https://www.google.com/maps/search/?api=1&query=${lat},${lon}`,
      "_blank",
      "noopener",
    );
    if (opened) {
      message.success?.("Abriendo la ruta en Google Maps");
    } else {
      message.error?.("No se pudo abrir el mapa");
    }
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard?.writeText(`${lat}, ${lon}`);
      message.success?.("Coordenadas copiadas");
    } catch {
      message.error?.("No se pudieron copiar las coordenadas");
    }
  };

  return (
    <div
      style={{
        padding: 12,
        border: "1px solid rgba(255,255,255,0.12)",
        borderRadius: 8,
        background: "rgba(255,255,255,0.03)",
        maxWidth: 420,
      }}
    >
      <Text strong style={{ display: "block" }}>
        {title ?? address ?? "Ubicación"}
      </Text>
      {address && (
        <Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
          {address}
        </Text>
      )}
      <div
        role="img"
        aria-label={`Mapa de la ubicación: ${title ?? address ?? `${lat}, ${lon}`}`}
        style={{
          height: 192,
          width: "100%",
          marginTop: 8,
          borderRadius: 6,
          overflow: "hidden",
        }}
      >
        <MapContainer
          key={`${lat},${lon}`}
          center={[lat, lon]}
          zoom={15}
          scrollWheelZoom={false}
          style={{ height: "100%", width: "100%" }}
        >
          <TileLayer url={TILE_URL} attribution={TILE_ATTRIBUTION} />
          <CircleMarker center={[lat, lon]} radius={8}>
            <Popup>
              {address
                ? `${title ?? "Ubicación"} — ${address}`
                : title ?? "Ubicación"}
            </Popup>
          </CircleMarker>
        </MapContainer>
      </div>
      {description && (
        <Text style={{ display: "block", marginTop: 8 }}>{description}</Text>
      )}
      <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
        <Button type="primary" onClick={handleSave} disabled={disabled}>
          Guardar
        </Button>
        <Button onClick={handleDirections} disabled={disabled}>
          Cómo llegar
        </Button>
        <Button
          onClick={() => {
            void handleCopy();
          }}
          disabled={disabled}
        >
          Copiar coordenadas
        </Button>
      </div>
    </div>
  );
};