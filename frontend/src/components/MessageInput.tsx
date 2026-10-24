import React, { useState, useImperativeHandle, forwardRef } from "react";
import { Input, Button } from "antd";
import { SendOutlined, LoadingOutlined } from "@ant-design/icons";

const { TextArea } = Input;

interface MessageInputProps {
  onSend: (content: string) => void;
  disabled: boolean;
}

export interface MessageInputHandle {
  focus: () => void;
  /**
   * Repone el contenido del campo desde fuera. Lo usa `ChatView` para devolver
   * al usuario el texto de un envío que el servidor rechazó (un `409` por
   * turno ocupado) en vez de darlo por perdido.
   */
  setValue: (value: string) => void;
}

export const MessageInput = forwardRef<MessageInputHandle, MessageInputProps>(
  ({ onSend, disabled }, ref) => {
    const [draft, setDraft] = useState("");
    const inputRef = React.useRef<React.ComponentRef<typeof TextArea>>(null);

    useImperativeHandle(ref, () => ({
      focus: () => inputRef.current?.focus(),
      setValue: (value: string) => setDraft(value),
    }));

    const handleSend = () => {
      if (draft.trim() && !disabled) {
        onSend(draft.trim());
        setDraft("");
      }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    };

    return (
      <div style={{ display: "flex", gap: 8, alignItems: "flex-end" }}>
        <TextArea
          ref={inputRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Escribe un mensaje a Valet..."
          autoSize={{ minRows: 2, maxRows: 6 }}
          disabled={disabled}
          style={{ flex: 1 }}
        />
        <Button
          type="primary"
          icon={disabled ? <LoadingOutlined /> : <SendOutlined />}
          onClick={handleSend}
          disabled={disabled || !draft.trim()}
          loading={disabled}
        />
      </div>
    );
  },
);

MessageInput.displayName = "MessageInput";
