import { CheckCircle2, Info, AlertTriangle, XCircle, X } from "lucide-react";
import { useToastStore, type ToastVariant } from "../../stores/useToastStore";
import "./ToastHost.css";

const ICONS: Record<ToastVariant, typeof Info> = {
  info: Info,
  success: CheckCircle2,
  warning: AlertTriangle,
  danger: XCircle,
};

export function ToastHost() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-host" role="status" aria-live="polite">
      {toasts.map((toast) => {
        const Icon = ICONS[toast.variant];
        return (
          <div key={toast.id} className={`toast toast--${toast.variant} anim-slide-up-toast`}>
            <Icon size={16} strokeWidth={2} className="toast__icon" />
            <span className="toast__message">{toast.message}</span>
            <button className="toast__close" onClick={() => dismiss(toast.id)} aria-label="Dismiss">
              <X size={13} strokeWidth={2} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
