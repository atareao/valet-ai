import React from "react";
import { Typography } from "antd";
import { InfoCircleOutlined, CodeOutlined } from "@ant-design/icons";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Message } from "../types";
import valetIcon from "../assets/valet-icon.svg";
import { UserAvatar } from "./UserAvatar";
import { WidgetRenderer } from "./widgets/WidgetRenderer";
import type { WidgetInstance } from "./widgets/types";

const { Text } = Typography;

function formatTime(isoDate: string): string {
  const date = new Date(isoDate);
  const hours = date.getHours().toString().padStart(2, "0");
  const minutes = date.getMinutes().toString().padStart(2, "0");
  return `${hours}:${minutes}`;
}

interface MessageBubbleProps {
  message: Message;
  userAvatarUrl?: string | null;
  widgets?: WidgetInstance[];
  onWidgetAction?: (
    widget: WidgetInstance,
    action: string,
    payload?: unknown,
  ) => void;
  widgetsDisabled?: boolean;
}

export const MessageBubble: React.FC<MessageBubbleProps> = ({
  message,
  userAvatarUrl,
  widgets,
  onWidgetAction,
  widgetsDisabled,
}) => {
  const config = getRoleConfig(message.role, userAvatarUrl);

  return (
    <div
      style={{
        display: "flex",
        justifyContent: config.justifyContent,
        marginBottom: 12,
        fontStyle: config.fontStyle,
        fontFamily: config.fontFamily,
      }}
    >
      {config.showIcon && config.iconPosition === "left" && (
        <div style={{ marginRight: 8, marginTop: 8 }}>{config.icon}</div>
      )}
      <div
        style={{
          maxWidth: "70%",
          padding: "10px 16px",
          borderRadius: 12,
          background: config.background,
          color: config.color,
          textAlign: config.textAlign as "left" | "center" | "right",
        }}
      >
        {message.role === "assistant" || message.role === "tool" ? (
          <div style={{ color: config.color, overflowX: "auto" }}>
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {message.content}
            </ReactMarkdown>
          </div>
        ) : (
          <Text style={{ color: config.color }}>{message.content}</Text>
        )}
        {widgets && widgets.length > 0 && (
          <div style={{ marginTop: 4 }}>
            {widgets.map((widget) => (
              <WidgetRenderer
                key={widget.id}
                id={widget.id}
                name={widget.name}
                data={widget.data}
                disabled={widgetsDisabled}
                onAction={(action, payload) =>
                  onWidgetAction?.(widget, action, payload)
                }
              />
            ))}
          </div>
        )}
        {/* Metadata lines */}
        <div style={{ fontSize: 10, marginTop: 4, opacity: 0.35 }}>
          {message.id !== "streaming" && (
            <div>{message.role} · {formatTime(message.created_at)}</div>
          )}
          {message.id !== "streaming" && message.location && (
            <div style={{ marginTop: 1 }}>📍 {message.location}</div>
          )}
          {message.id !== "streaming" && message.tools_used && (
            <div style={{ marginTop: 1 }}>🔧 {message.tools_used}</div>
          )}
        </div>
      </div>
      {config.showIcon && config.iconPosition === "right" && (
        <div style={{ marginLeft: 8, marginTop: 8 }}>{config.icon}</div>
      )}
    </div>
  );
};

interface RoleConfig {
  justifyContent: string;
  background: string;
  color: string;
  fontStyle: string;
  fontFamily: string;
  textAlign: string;
  showIcon: boolean;
  iconPosition: "left" | "right";
  icon: React.ReactNode;
}

function getRoleConfig(
  role: string,
  userAvatarUrl?: string | null,
): RoleConfig {
  switch (role) {
    case "user":
      return {
        justifyContent: "flex-end",
        background: "#1677ff",
        color: "#fff",
        fontStyle: "normal",
        fontFamily: "inherit",
        textAlign: "left",
        showIcon: true,
        iconPosition: "right" as const,
        icon: <UserAvatar src={userAvatarUrl} />,
      };
    case "assistant":
      return {
        justifyContent: "flex-start",
        background: "rgba(255,255,255,0.06)",
        color: "#fff",
        fontStyle: "normal",
        fontFamily: "inherit",
        textAlign: "left",
        showIcon: true,
        iconPosition: "left" as const,
        icon: (
          <img
            src={valetIcon}
            alt="Valet"
            width={24}
            height={24}
            style={{ display: "block" }}
          />
        ),
      };
    case "system":
      return {
        justifyContent: "center",
        background: "transparent",
        color: "rgba(255,255,255,0.45)",
        fontStyle: "italic",
        fontFamily: "inherit",
        textAlign: "center",
        showIcon: true,
        iconPosition: "left" as const,
        icon: (
          <InfoCircleOutlined style={{ color: "rgba(255,255,255,0.45)" }} />
        ),
      };
    case "tool":
      return {
        justifyContent: "flex-start",
        background: "rgba(255,255,255,0.03)",
        color: "rgba(255,255,255,0.75)",
        fontStyle: "normal",
        fontFamily: "monospace",
        textAlign: "left",
        showIcon: true,
        iconPosition: "left" as const,
        icon: <CodeOutlined style={{ color: "rgba(255,255,255,0.45)" }} />,
      };
    default:
      return {
        justifyContent: "flex-start",
        background: "rgba(255,255,255,0.06)",
        color: "#fff",
        fontStyle: "normal",
        fontFamily: "inherit",
        textAlign: "left",
        showIcon: false,
        iconPosition: "left" as const,
        icon: null,
      };
  }
}
