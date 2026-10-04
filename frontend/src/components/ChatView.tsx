import React, { useRef, useEffect, useMemo } from "react";
import { Modal, Button, Typography } from "antd";
import type { Message } from "../types";
import { MessageBubble } from "./MessageBubble";
import { DateSeparator } from "./DateSeparator";
import { MessageInput, type MessageInputHandle } from "./MessageInput";
import type { WidgetInstance } from "./widgets/types";

interface ChatViewProps {
  messages: Message[];
  loading: boolean;
  onSendMessage: (content: string) => void;
  streaming?: boolean;
  streamingContent?: string;
  activeTools?: string[];
  settings?: Record<string, string> | null;
  userAvatarUrl?: string | null;
  pendingApproval?: {
    requestId: string;
    toolName: string;
    reason: string;
  } | null;
  onResolveApproval?: (approved: boolean) => void;
  widgetsByMessage?: Record<string, WidgetInstance[]>;
  onWidgetAction?: (
    widget: WidgetInstance,
    action: string,
    payload?: unknown,
  ) => void;
}

function isSameDay(date1: string, date2: string): boolean {
  const d1 = new Date(date1);
  const d2 = new Date(date2);
  return (
    d1.getFullYear() === d2.getFullYear() &&
    d1.getMonth() === d2.getMonth() &&
    d1.getDate() === d2.getDate()
  );
}

export const ChatView: React.FC<ChatViewProps> = (props) => {
  const {
    messages,
    loading,
    onSendMessage,
    streaming,
    streamingContent,
    activeTools,
    userAvatarUrl,
    pendingApproval,
    onResolveApproval,
    widgetsByMessage,
    onWidgetAction,
  } = props;
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<MessageInputHandle>(null);

  const allMessages = useMemo(
    () =>
      streaming && streamingContent
        ? [
            ...messages,
            {
              id: "streaming",
              role: "assistant" as const,
              content: streamingContent,
              created_at: new Date().toISOString(),
            },
          ]
        : messages,
    [messages, streaming, streamingContent],
  );

  // Auto-scroll to bottom when messages or streaming content changes
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [allMessages, streamingContent]);

  // Focus input when streaming ends
  useEffect(() => {
    if (!streaming) {
      inputRef.current?.focus();
    }
  }, [streaming]);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "calc(100vh - 64px)",
      }}
    >
      <div
        style={{
          flex: 1,
          overflow: "auto",
          padding: "16px 0",
          scrollBehavior: "smooth",
        }}
      >
        {loading && <div>Cargando...</div>}
        {allMessages.map((msg, idx) => {
          const prevMsg = idx > 0 ? allMessages[idx - 1] : null;
          const showSeparator = !prevMsg || !isSameDay(prevMsg.created_at, msg.created_at);
          return (
            <React.Fragment key={msg.id}>
              {showSeparator && <DateSeparator date={msg.created_at} />}
              <MessageBubble
                message={msg}
                userAvatarUrl={userAvatarUrl}
                widgets={widgetsByMessage?.[msg.id]}
                onWidgetAction={onWidgetAction}
                widgetsDisabled={streaming}
              />
            </React.Fragment>
          );
        })}
        <div ref={bottomRef} />
        {allMessages.length === 0 && !loading && (
          <div
            style={{
              textAlign: "center",
              marginTop: "40vh",
              color: "rgba(255,255,255,0.45)",
            }}
          >
            Inicia una conversación con Valet
          </div>
        )}
      </div>
      {activeTools && activeTools.length > 0 && (
        <div
          style={{
            padding: "8px 24px",
            background: "rgba(255, 255, 255, 0.03)",
            borderTop: "1px solid rgba(255,255,255,0.1)",
            color: "rgba(255,255,255,0.65)",
            fontSize: 13,
            display: "flex",
            gap: 8,
            alignItems: "center",
          }}
        >
          <span>🔧</span>
          {activeTools.map((tool, i) => (
            <span key={i}>
              {i > 0 && <span style={{ margin: "0 4px" }}>·</span>}
              <code
                style={{
                  background: "rgba(255,255,255,0.08)",
                  padding: "2px 6px",
                  borderRadius: 4,
                  fontSize: 12,
                }}
              >
                {tool}
              </code>
            </span>
          ))}
          <span style={{ marginLeft: "auto", fontSize: 11 }}>
            ejecutando...
          </span>
        </div>
      )}
      <div
        style={{ padding: 16, borderTop: "1px solid rgba(255,255,255,0.1)" }}
      >
        <MessageInput
          ref={inputRef}
          onSend={onSendMessage}
          disabled={loading || !!streaming}
        />
      </div>
      <Modal
        title="⚠️ Confirmación requerida"
        open={!!pendingApproval}
        onCancel={() => onResolveApproval?.(false)}
        closable={false}
        maskClosable={false}
        footer={[
          <Button
            key="deny"
            onClick={() => onResolveApproval?.(false)}
          >
            Denegar
          </Button>,
          <Button
            key="allow"
            type="primary"
            danger
            onClick={() => onResolveApproval?.(true)}
          >
            Permitir
          </Button>,
        ]}
      >
        <p>Se ha solicitado ejecutar una herramienta destructiva:</p>
        <Typography.Text code>
          {pendingApproval?.toolName}
        </Typography.Text>
        {pendingApproval?.reason && (
          <p style={{ marginTop: 12, marginBottom: 0 }}>
            {pendingApproval.reason}
          </p>
        )}
      </Modal>
    </div>
  );
};
