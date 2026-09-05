import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import "./ToolModal.css";

interface ToolModalProps {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}

/** Shared modal shell for every Tools-screen panel — same overlay, scale-in,
 * header, and scroll body as `FormatPicker`, just generalized for richer
 * per-tool content instead of a single list. */
export function ToolModal({ title, subtitle, onClose, children, footer, width = 480 }: ToolModalProps) {
  return createPortal(
    <div className="tool-modal-overlay anim-fade-in" onMouseDown={onClose}>
      <div
        className="tool-modal anim-scale-in"
        style={{ width }}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="tool-modal__header">
          <div>
            <h3>{title}</h3>
            {subtitle && <p className="tool-modal__subtitle">{subtitle}</p>}
          </div>
          <button className="tool-modal__close" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>
        <div className="tool-modal__body">{children}</div>
        {footer && <div className="tool-modal__footer">{footer}</div>}
      </div>
    </div>,
    document.body
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="tool-modal__field">
      <label className="tool-modal__label">{label}</label>
      {children}
    </div>
  );
}
