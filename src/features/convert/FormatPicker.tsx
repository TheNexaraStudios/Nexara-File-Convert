import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { Search, X, Check } from "lucide-react";
import type { FormatInfo } from "../../types/format";
import { CATEGORY_LABELS } from "../../types/format";
import "./FormatPicker.css";

interface FormatPickerProps {
  options: FormatInfo[];
  selectedFormatId: string | null;
  onSelect: (formatId: string) => void;
  onClose: () => void;
}

export function FormatPicker({ options, selectedFormatId, onSelect, onClose }: FormatPickerProps) {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return options;
    return options.filter(
      (f) => f.name.toLowerCase().includes(q) || f.id.toLowerCase().includes(q) || f.extensions.some((e) => e.includes(q))
    );
  }, [options, query]);

  const recommended = filtered.slice(0, 3);
  const rest = filtered.slice(3);

  const groups = useMemo(() => {
    const map = new Map<string, FormatInfo[]>();
    for (const fmt of rest) {
      const list = map.get(fmt.category) ?? [];
      list.push(fmt);
      map.set(fmt.category, list);
    }
    return Array.from(map.entries());
  }, [rest]);

  return createPortal(
    <div className="format-picker-overlay anim-fade-in" onMouseDown={onClose}>
      <div
        className="format-picker anim-scale-in"
        role="dialog"
        aria-modal="true"
        aria-label="Choose output format"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="format-picker__header">
          <h3>Convert to</h3>
          <button className="format-picker__close" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <div className="format-picker__search">
          <Search size={15} strokeWidth={2} />
          <input
            autoFocus
            type="text"
            placeholder="Search formats..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>

        <div className="format-picker__list">
          {filtered.length === 0 && (
            <div className="format-picker__empty">No compatible formats match "{query}"</div>
          )}

          {recommended.length > 0 && (
            <FormatSection
              title="Recommended"
              formats={recommended}
              selectedFormatId={selectedFormatId}
              onSelect={onSelect}
            />
          )}

          {groups.map(([category, formats]) => (
            <FormatSection
              key={category}
              title={CATEGORY_LABELS[category as keyof typeof CATEGORY_LABELS] ?? category}
              formats={formats}
              selectedFormatId={selectedFormatId}
              onSelect={onSelect}
            />
          ))}
        </div>
      </div>
    </div>,
    document.body
  );
}

function FormatSection({
  title,
  formats,
  selectedFormatId,
  onSelect,
}: {
  title: string;
  formats: FormatInfo[];
  selectedFormatId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="format-picker__section">
      <div className="format-picker__section-title">{title}</div>
      {formats.map((fmt) => (
        <button
          key={fmt.id}
          className={`format-picker__option ${selectedFormatId === fmt.id ? "format-picker__option--selected" : ""}`}
          onClick={() => onSelect(fmt.id)}
        >
          <span className="format-picker__option-ext">{fmt.id.toUpperCase()}</span>
          <span className="format-picker__option-name">{fmt.name}</span>
          {selectedFormatId === fmt.id && <Check size={15} className="format-picker__option-check" />}
        </button>
      ))}
    </div>
  );
}
