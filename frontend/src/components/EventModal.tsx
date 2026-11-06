import React, { useEffect } from "react";
import {
  Modal,
  Form,
  Input,
  DatePicker,
  Switch,
  Select,
  InputNumber,
  App as AntdApp,
} from "antd";
import dayjs from "dayjs";
import type { Dayjs } from "dayjs";
import type { CalendarEvent } from "../types";
import { useCreateEvent, useUpdateEvent } from "../hooks/useEvents";
import { useMediaQuery } from "../hooks/useMediaQuery";

const { TextArea } = Input;

const CATEGORY_OPTIONS = [
  { value: "default", label: "Default" },
  { value: "work", label: "Work" },
  { value: "personal", label: "Personal" },
  { value: "health", label: "Health" },
  { value: "birthday", label: "Birthday" },
  { value: "holiday", label: "Holiday" },
];

const SCOPE_OPTIONS = [
  { value: "shared", label: "Shared" },
  { value: "personal", label: "Personal" },
];

interface EventModalProps {
  open: boolean;
  event: CalendarEvent | null;
  defaultDate: Dayjs | null;
  onClose: () => void;
  onSaved: () => void;
}

export const EventModal: React.FC<EventModalProps> = ({
  open,
  event,
  defaultDate,
  onClose,
  onSaved,
}) => {
  const [form] = Form.useForm();
  const { message: messageApi } = AntdApp.useApp();
  const { create, loading: creating } = useCreateEvent();
  const { update, loading: updating } = useUpdateEvent();
  const isEditing = !!event;
  const isMobile = useMediaQuery("(max-width: 767px)");

  useEffect(() => {
    if (open) {
      if (event) {
        form.setFieldsValue({
          title: event.title,
          start_time: dayjs(event.start_time),
          end_time: dayjs(event.end_time),
          all_day: event.all_day,
          category: event.category,
          scope: event.scope,
          rrule: event.rrule || "",
          reminder_minutes_before: event.reminder_minutes_before,
          description: event.description || "",
          location: event.location || "",
        });
      } else {
        form.resetFields();
        if (defaultDate) {
          form.setFieldsValue({
            start_time: defaultDate.hour(9).minute(0).second(0),
            end_time: defaultDate.hour(10).minute(0).second(0),
            all_day: false,
            category: "default",
            scope: "personal",
          });
        }
      }
    }
  }, [open, event, defaultDate, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      const data: Partial<CalendarEvent> = {
        title: values.title,
        start_time: values.start_time.toISOString(),
        end_time: values.end_time.toISOString(),
        all_day: values.all_day || false,
        category: values.category || "default",
        scope: values.scope || "personal",
        rrule: values.rrule || undefined,
        reminder_minutes_before: values.reminder_minutes_before || undefined,
        description: values.description || undefined,
        location: values.location || undefined,
      };

      if (isEditing && event) {
        await update(event.id, data);
        messageApi.success("Event updated");
      } else {
        await create(data);
        messageApi.success("Event created");
      }
      onSaved();
    } catch (err) {
      if (err && typeof err === "object" && "errorFields" in err) {
        // Form validation error — antd shows inline errors
        return;
      }
      const msg = err instanceof Error ? err.message : "Failed to save event";
      messageApi.error(msg);
    }
  };

  return (
    <Modal
      title={isEditing ? "Edit Event" : "New Event"}
      open={open}
      onOk={handleSubmit}
      onCancel={onClose}
      confirmLoading={creating || updating}
      destroyOnHidden
      width={isMobile ? "calc(100vw - 32px)" : 520}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          all_day: false,
          category: "default",
          scope: "personal",
        }}
      >
        <Form.Item
          name="title"
          label="Title"
          rules={[{ required: true, message: "Please enter a title" }]}
        >
          <Input placeholder="Event title" />
        </Form.Item>

        <Form.Item
          name="start_time"
          label="Start"
          rules={[{ required: true, message: "Please select start time" }]}
        >
          <DatePicker showTime style={{ width: "100%" }} />
        </Form.Item>

        <Form.Item
          name="end_time"
          label="End"
          rules={[{ required: true, message: "Please select end time" }]}
        >
          <DatePicker showTime style={{ width: "100%" }} />
        </Form.Item>

        <Form.Item name="all_day" label="All Day" valuePropName="checked">
          <Switch />
        </Form.Item>

        <Form.Item name="category" label="Category">
          <Select options={CATEGORY_OPTIONS} />
        </Form.Item>

        <Form.Item name="scope" label="Scope">
          <Select options={SCOPE_OPTIONS} />
        </Form.Item>

        <Form.Item name="rrule" label="Recurrence Rule (RRULE)">
          <Input placeholder="e.g. FREQ=WEEKLY;BYDAY=MO" />
        </Form.Item>

        <Form.Item
          name="reminder_minutes_before"
          label="Reminder (minutes before)"
        >
          <InputNumber min={0} style={{ width: "100%" }} />
        </Form.Item>

        <Form.Item name="description" label="Description">
          <TextArea rows={3} placeholder="Event description" />
        </Form.Item>

        <Form.Item name="location" label="Location">
          <Input placeholder="Event location" />
        </Form.Item>
      </Form>
    </Modal>
  );
};
